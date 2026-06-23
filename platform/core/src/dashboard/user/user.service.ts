import {
  BadRequestException,
  ConflictException,
  forwardRef,
  Inject,
  Injectable,
  Logger
} from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { Transaction } from 'neo4j-driver'
import { uuidv7 } from 'uuidv7'

import { getCurrentISO } from '@/common/utils/getCurrentISO'
import { isDevMode } from '@/common/utils/isDevMode'
import { toBoolean } from '@/common/utils/toBolean'
import { removeUndefinedKeys } from '@/core/property/property.utils'
import { IDecodedResetToken } from '@/dashboard/auth/auth.types'
import { ResetPasswordAuthDto } from '@/dashboard/auth/dto/reset-password-auth.dto'
import { EncryptionService } from '@/dashboard/auth/encryption/encryption.service'
import { ProjectService } from '@/dashboard/project/project.service'
import { ICreatedUserData } from '@/dashboard/user/interfaces/authenticated-user.interface'
import { AcceptWorkspaceInvitationParams } from '@/dashboard/user/interfaces/user-properties.interface'
import {
  USER_ROLE_EDITOR,
  USER_ROLE_OWNER,
  USER_ROLE_WEIGHT
} from '@/dashboard/user/interfaces/user.constants'
import { sanitizeSettings, validateEmail } from '@/dashboard/user/user.utils'
import { WorkspaceService } from '@/dashboard/workspace/workspace.service'
import { TWorkSpaceInviteToken } from '@/dashboard/workspace/workspace.types'

import * as crypto from 'node:crypto'

import { TUserProperties, TUserRoles } from './model/user.interface'
import { UserRepository } from './model/user.repository'
import { User } from './user.entity'

import type { UserRow } from '@/database/sql/schema/types'

@Injectable()
export class UserService {
  constructor(
    private readonly configService: ConfigService,
    private readonly encryptionService: EncryptionService,
    private readonly userRepository: UserRepository,
    @Inject(forwardRef(() => WorkspaceService))
    private readonly workspaceService: WorkspaceService,
    @Inject(forwardRef(() => ProjectService))
    private readonly projectService: ProjectService
  ) {}

  normalize(row?: UserRow): User | undefined {
    return row ? new User(row) : undefined
  }

  async find(login: string, _transaction?: Transaction): Promise<User | undefined> {
    const row = await this.userRepository.findByLogin(login)
    return this.normalize(row)
  }

  async findById(id: string, _transaction?: Transaction): Promise<User | undefined> {
    const row = await this.userRepository.findById(id)
    return this.normalize(row)
  }

  /** @deprecated – kept for call-site compatibility; returns plain UserRow */
  async findUserNodeById(id: string, _transaction?: Transaction): Promise<UserRow | undefined> {
    return this.userRepository.findById(id)
  }

  /** @deprecated – kept for call-site compatibility; returns plain UserRow */
  async findUserNodeByLogin(login: string, _transaction?: Transaction): Promise<UserRow | undefined> {
    return this.userRepository.findByLogin(login)
  }

  async markEmailAsConfirmed(login: string, _transaction?: Transaction): Promise<User> {
    const existing = await this.userRepository.findByLogin(login)
    if (!existing) {
      throw new BadRequestException(`User not found: ${login}`)
    }
    const updated = await this.userRepository.update(existing.id, { confirmed: true })
    return this.normalize(updated)
  }

  async create(
    properties: Omit<TUserProperties, 'id' | 'isEmail'>,
    transaction?: Transaction
  ): Promise<ICreatedUserData> {
    const allowedLogins = JSON.parse(this.configService.get('RUSHDB_ALLOWED_LOGINS') || '[]') ?? []

    if (allowedLogins.length === 0 || allowedLogins.includes(properties.login)) {
      const userRow = await this.createUserNode(properties, transaction)
      await this.workspaceService.createWorkspace({ name: 'Default Workspace' }, userRow.id, transaction)

      return { userData: this.normalize(userRow) }
    } else {
      throw new BadRequestException('Provided login is not allowed')
    }
  }

  /**
   * Just-in-time user creation for Enterprise SSO. Unlike {@link create}, this
   * does NOT spin up a personal "Default Workspace" — the caller (SSO service)
   * is responsible for attaching the user to the IdP's workspace with the
   * mapped role. The login allowlist is intentionally bypassed because the
   * verified IdP domain already acts as the allowlist.
   */
  async createSsoUser(
    properties: Omit<TUserProperties, 'id' | 'isEmail'>,
    transaction?: Transaction
  ): Promise<User> {
    const userRow = await this.createUserNode({ ...properties, confirmed: true }, transaction)
    return this.normalize(userRow)
  }

  async acceptWorkspaceInvitation(
    params: AcceptWorkspaceInvitationParams,
    transaction?: Transaction
  ): Promise<ICreatedUserData> {
    const allowedLogins = JSON.parse(this.configService.get('RUSHDB_ALLOWED_LOGINS') || '[]') ?? []
    const { inviteToken } = params
    const { workspaceId, email, projectIds } = this.decryptInvite(inviteToken)

    const login = email
    const providedUserLogin = params.authUserLogin

    if (email !== providedUserLogin) {
      throw new BadRequestException('Invitation was provided to another RushDB user')
    }

    isDevMode(() => Logger.log(`[Accept invite LOG]: Fetching pending invites for workspace ${workspaceId}`))
    const pending = await this.workspaceService.getPendingInvites(workspaceId, transaction)

    if (!pending.some((inv) => inv.email === login && inv.email === providedUserLogin)) {
      isDevMode(() =>
        Logger.warn(`[Accept invite WARN]: No pending invite for ${login} in workspace ${workspaceId}`)
      )
      throw new BadRequestException('No pending invitation found for this email')
    }

    if (allowedLogins.length === 0 || allowedLogins.includes(login)) {
      const userRow = await this.findUserNodeByLogin(login, transaction)

      if (!workspaceId || !email) {
        throw new BadRequestException('Malformed invite provided')
      }
      if (email !== login) {
        throw new BadRequestException("Provided email doesn't match invitee's email")
      }

      await this.workspaceService.attachUserToWorkspace(
        workspaceId,
        userRow.id,
        USER_ROLE_EDITOR,
        transaction
      )

      isDevMode(() =>
        Logger.log(`[Accept user invitation LOG]: User accepted invitation and created ${userRow.id}`)
      )

      if (Array.isArray(projectIds) && projectIds.length > 0) {
        for (const projectId of projectIds) {
          await this.linkUser(userRow.id, projectId, transaction)
        }
      }

      await this.workspaceService.removePendingInvite(workspaceId, login, transaction)
      isDevMode(() => Logger.log(`[Accept invite LOG] removed pending invite for ${login}`))

      return { userData: this.normalize(userRow), workspaceId }
    } else {
      throw new BadRequestException('Provided login is not allowed')
    }
  }

  decryptInvite(encrypted: string): TWorkSpaceInviteToken {
    const tokenNormalized = decodeURIComponent(encrypted)
    const encryptionKey = this.configService.get('RUSHDB_AES_256_ENCRYPTION_KEY')
    const iv = tokenNormalized.substring(0, 32)
    const cipherText = tokenNormalized.substring(32)
    const decipher = crypto.createDecipheriv('aes-256-cbc', encryptionKey, Buffer.from(iv, 'hex'))

    let decrypted: string
    try {
      decrypted = decipher.update(cipherText, 'base64', 'utf8')
      decrypted = decrypted + decipher.final('utf8')
    } catch {
      throw new BadRequestException('Invalid invitation token')
    }
    return JSON.parse(decrypted) as TWorkSpaceInviteToken
  }

  async createUserNode(
    properties: Omit<TUserProperties, 'id' | 'isEmail'>,
    _transaction?: Transaction
  ): Promise<UserRow> {
    let userSettings = ''
    const currentTime = getCurrentISO()
    const userId = uuidv7()
    const userPassword = await this.encryptionService.hash(properties.password)

    if (properties.settings) {
      userSettings = sanitizeSettings(properties.settings)
    }

    const isEmail = toBoolean(validateEmail(properties.login || ''))
    const isOnlyEmailLoginAllowed = !toBoolean(this.configService.get('RUSHDB_SELF_HOSTED'))

    if (!isEmail && isOnlyEmailLoginAllowed) {
      isDevMode(() => Logger.error('[Create user ERROR]: Bad email provided'))
      throw new BadRequestException('Bad email data provided')
    }

    try {
      return await this.userRepository.create({
        id: userId,
        login: properties.login,
        isEmail,
        password: userPassword,
        confirmed: properties.confirmed ?? false,
        created: currentTime,
        firstName: properties.firstName,
        lastName: properties.lastName,
        settings: userSettings || null,
        status: properties.status,
        googleAuth: properties.googleAuth,
        githubAuth: properties.githubAuth,
        samlAuth: properties.samlAuth,
        oidcAuth: properties.oidcAuth
      })
    } catch {
      throw new ConflictException('Provided email is registered already')
    }
  }

  async update(
    id: string,
    userProperties: Partial<TUserProperties>,
    _transaction?: Transaction
  ): Promise<User> {
    const existing = await this.userRepository.findById(id)
    if (!existing) {
      throw new BadRequestException(`User ${id} not found`)
    }

    const fieldsToUpdate = removeUndefinedKeys(userProperties) as any

    // Sanitize settings if present
    if (fieldsToUpdate.settings) {
      fieldsToUpdate.settings = sanitizeSettings(fieldsToUpdate.settings)
    }

    // Map TUserProperties camelCase to SQL column names
    const sqlFields: Record<string, any> = {}
    const fieldMap: Record<string, string> = {
      firstName: 'firstName',
      lastName: 'lastName',
      confirmed: 'confirmed',
      status: 'status',
      settings: 'settings',
      lastActivity: 'lastActivity',
      googleAuth: 'googleAuth',
      githubAuth: 'githubAuth',
      samlAuth: 'samlAuth',
      oidcAuth: 'oidcAuth',
      password: 'password',
      deletedDate: 'deletedDate'
    }
    for (const [key, val] of Object.entries(fieldsToUpdate)) {
      if (fieldMap[key]) {
        sqlFields[fieldMap[key]] = val
      }
    }
    sqlFields['edited'] = getCurrentISO()

    const updated = await this.userRepository.update(id, sqlFields)
    return this.normalize(updated)
  }

  async resetUserPassword(
    newPasswordData: ResetPasswordAuthDto,
    tokenPayload: IDecodedResetToken,
    transaction?: Transaction
  ): Promise<User> {
    const user = await this.find(newPasswordData.login, transaction)
    const userJson = user?.toJson()

    if (userJson?.login !== tokenPayload.login || userJson?.id !== tokenPayload.id) {
      throw new BadRequestException('Confirmation data malformed')
    }

    const newPasswordEncrypted = await this.encryptionService.hash(newPasswordData.password)
    return this.update(userJson.id, { password: newPasswordEncrypted }, transaction)
  }

  async hasMinimalAccessLevel({
    userId,
    targetId,
    targetType = 'workspace',
    accessLevel,
    transaction: _transaction
  }: {
    userId: string
    targetId: string
    targetType?: 'project' | 'workspace'
    accessLevel?: TUserRoles
    transaction?: Transaction
  }): Promise<boolean> {
    let currentRole: TUserRoles | undefined

    if (targetType === 'workspace') {
      currentRole = (await this.userRepository.getUserRoleInWorkspace(userId, targetId)) as TUserRoles
    } else {
      currentRole = (await this.userRepository.getUserRoleInProject(userId, targetId)) as TUserRoles
    }

    if (!currentRole) {
      return false
    }

    const requiredWeight = USER_ROLE_WEIGHT[accessLevel] ?? 0
    const currentWeight = USER_ROLE_WEIGHT[currentRole] ?? 0
    return currentWeight >= requiredWeight
  }

  async delete({ userId, transaction }: { userId: string; transaction?: Transaction }): Promise<true> {
    const userRow = await this.userRepository.findById(userId)
    if (!userRow) {
      return true
    }

    // Delete owned workspaces first
    const workspaceRows = await this.userRepository.findWorkspacesForUser(userId)
    for (const { workspace, role } of workspaceRows) {
      if (role === USER_ROLE_OWNER) {
        await this.workspaceService.deleteWorkspace(workspace.id, transaction)
      }
    }

    await this.userRepository.delete(userId)
    return true
  }

  async linkUser(id: string, projectId: string, _transaction?: Transaction): Promise<boolean> {
    const currentTime = getCurrentISO()

    try {
      const project = await this.projectService.getProjectById(projectId)
      if (!project) {
        return false
      }
    } catch {
      isDevMode(() =>
        Logger.warn(`[Link user to the project WARN]: Incorrect project id provided ${projectId}`)
      )
      return false
    }

    try {
      await this.projectService.linkUserToProject(id, projectId, currentTime, _transaction)
      isDevMode(() =>
        Logger.log(`[Link user ${id} to the project LOG]: User linked to the project ${projectId}`)
      )
    } catch (e) {
      isDevMode(() => Logger.error('[Link user to the project ERROR]: Error while linking user', e))
      throw new BadRequestException('Cant link user to project')
    }

    return true
  }

  async getUserWorkspaceRole(id: string, workspaceId: string, _transaction?: Transaction) {
    return this.workspaceService.getUserRoleInWorkspaceById(id, workspaceId, _transaction)
  }
}
