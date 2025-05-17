import { BadRequestException, forwardRef, Inject, Injectable, Logger } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { Transaction } from 'neo4j-driver'
import { uuidv7 } from 'uuidv7'

import { QueryBuilder } from '@/common/QueryBuilder'
import { getCurrentISO } from '@/common/utils/getCurrentISO'
import { isDevMode } from '@/common/utils/isDevMode'
import { toBoolean } from '@/common/utils/toBolean'
import { removeUndefinedKeys } from '@/core/property/property.utils'
import { IDecodedResetToken } from '@/dashboard/auth/auth.types'
import { ResetPasswordAuthDto } from '@/dashboard/auth/dto/reset-password-auth.dto'
import { EncryptionService } from '@/dashboard/auth/encryption/encryption.service'
import { StripeService } from '@/dashboard/billing/stripe/stripe.service'
import {
  RUSHDB_LABEL_PROJECT,
  RUSHDB_LABEL_USER,
  RUSHDB_LABEL_WORKSPACE,
  RUSHDB_RELATION_HAS_ACCESS,
  RUSHDB_RELATION_MEMBER_OF
} from '@/dashboard/common/constants'
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
import { NeogmaService } from '@/database/neogma/neogma.service'

import * as crypto from 'node:crypto'

import { TUserInstance, TUserProperties, TUserRoles } from './model/user.interface'
import { UserRepository } from './model/user.repository'
import { User } from './user.entity'

@Injectable()
export class UserService {
  constructor(
    private readonly configService: ConfigService,
    private readonly neogmaService: NeogmaService,
    private readonly encryptionService: EncryptionService,
    private readonly userRepository: UserRepository,
    @Inject(forwardRef(() => WorkspaceService))
    private readonly workspaceService: WorkspaceService,
    @Inject(forwardRef(() => StripeService))
    private readonly stripeService: StripeService,
    @Inject(forwardRef(() => ProjectService))
    private readonly projectService: ProjectService
  ) {}

  normalize(node?: TUserInstance) {
    return node ? new User(node) : undefined
  }

  async find(login: string, transaction: Transaction): Promise<User | undefined> {
    const node = await this.userRepository.model.findOne({
      where: { login },
      throwIfNotFound: false,
      session: transaction
    })

    return this.normalize(node)
  }

  async findById(id: string, transaction: Transaction): Promise<User | undefined> {
    const node = await this.userRepository.model.findOne({
      where: { id },
      throwIfNotFound: false,
      session: transaction
    })

    return this.normalize(node)
  }

  async findUserNodeById(id: string, transaction: Transaction) {
    const queryRunner = this.neogmaService.createRunner()

    const findUser = await this.neogmaService
      .createBuilder()
      .match({
        model: this.userRepository.model,
        where: { id },
        identifier: 'i'
      })
      .return('i')
      .run(queryRunner, transaction)

    const user = findUser.records[0]?.get('i')

    if (!user) {
      return
    }

    return this.userRepository.model.buildFromRecord(user)
  }

  async findUserNodeByLogin(login: string, transaction: Transaction) {
    const queryRunner = this.neogmaService.createRunner()

    const findUser = await this.neogmaService
      .createBuilder()
      .match({
        model: this.userRepository.model,
        where: { login },
        identifier: 'i'
      })
      .return('i')
      .run(queryRunner, transaction)

    const user = findUser.records[0]?.get('i')

    if (!user) {
      return
    }

    return this.userRepository.model.buildFromRecord(user)
  }

  async markEmailAsConfirmed(login: string, transaction: Transaction): Promise<User> {
    const queryRunner = this.neogmaService.createRunner()

    const result = await queryRunner.run(
      `
                MATCH (u:${RUSHDB_LABEL_USER} { login: $login })
                SET u.confirmed = true
                RETURN u`,
      {
        login
      },
      transaction
    )

    return result?.records?.map((v) => v.get('u'))[0]
  }

  async create(
    properties: Omit<TUserProperties, 'id' | 'isEmail'>,
    transaction: Transaction
  ): Promise<ICreatedUserData> {
    const allowedLogins = JSON.parse(this.configService.get('RUSHDB_ALLOWED_LOGINS') || '[]') ?? []

    if (allowedLogins.length === 0 || (allowedLogins.length && allowedLogins.includes(properties.login))) {
      const userNode = await this.createUserNode(properties, transaction)
      // Add Default Workspace on registration
      await this.workspaceService.createWorkspace({ name: 'Default Workspace' }, userNode.id, transaction)

      if (!toBoolean(this.configService.get('RUSHDB_SELF_HOSTED'))) {
        await this.stripeService.createCustomer(properties.login)
      }

      return {
        userData: this.normalize(userNode)
      }
    } else {
      throw new BadRequestException('Provided login is not allowed')
    }
  }

  async acceptWorkspaceInvitation<T extends boolean = boolean>(
    params: AcceptWorkspaceInvitationParams<T>,
    transaction: Transaction
  ): Promise<ICreatedUserData> {
    const allowedLogins = JSON.parse(this.configService.get('RUSHDB_ALLOWED_LOGINS') || '[]') ?? []
    const { inviteToken, forceUserSignUp } = params
    const { workspaceId, email, projectIds } = this.decryptInvite(inviteToken)

    const login = forceUserSignUp === true ? params.userData.login : email
    const providedUserLogin = forceUserSignUp === false ? params.authUserLogin : null

    if (!forceUserSignUp && email !== providedUserLogin) {
      throw new BadRequestException('Invitation was provided to another RushDB user')
    }

    isDevMode(() => Logger.log(`[Accept invite LOG]: Fetching pending invites for workspace ${workspaceId}`))
    const pending = await this.workspaceService.getPendingInvites(workspaceId, transaction)

    if (!pending.some((inv) => inv.email === login)) {
      isDevMode(() =>
        Logger.warn(`[Accept invite WARN]: No pending invite for ${login} in workspace ${workspaceId}`)
      )

      throw new BadRequestException('No pending invitation found for this email')
    }

    if (allowedLogins.length === 0 || (allowedLogins.length && allowedLogins.includes(login))) {
      // For OAuth we don't abort request early bc we want to check google oauth first
      let shouldReCheckUser = false

      if (!forceUserSignUp) {
        isDevMode(() => Logger.warn(`[Accept user invitation WARN]: User with potentially malformed request`))

        // Mark user with potentially malformed data or user with google oauth
        shouldReCheckUser = true
      }

      let userNode

      if (forceUserSignUp) {
        isDevMode(() => Logger.log(`[Accept user invitation LOG]: Create user node for ${login}`))
        userNode = await this.createUserNode(params.userData, transaction)

        isDevMode(() => Logger.log(`[Accept user invitation LOG]: Create default workspace for ${login}`))
        await this.workspaceService.createWorkspace({ name: 'Default Workspace' }, userNode.id, transaction)
      } else {
        userNode = await this.findUserNodeByLogin(login, transaction)

        if (shouldReCheckUser && !userNode.googleAuth) {
          throw new BadRequestException('Invitation was provided to a new RushDB user')
        } else if (shouldReCheckUser) {
          isDevMode(() =>
            Logger.log(`[Accept user invitation LOG]: User ${userNode.id} registered before with oauth`)
          )
        }
      }

      if (!workspaceId || !email) {
        throw new BadRequestException('Malformed invite provided')
      }

      if (email !== login) {
        throw new BadRequestException("Provided email doesn't match invitee's email")
      }

      await this.workspaceService.attachUserToWorkspace(
        workspaceId,
        userNode.id,
        USER_ROLE_EDITOR,
        transaction
      )

      isDevMode(() =>
        Logger.log(`[Accept user invitation LOG]: User accepted invitation and created ${userNode.id}`)
      )

      if (Array.isArray(projectIds) && projectIds.length > 0) {
        for (const projectId of projectIds) {
          await this.linkUser(userNode.id, projectId, transaction)
        }
      }

      if (!toBoolean(this.configService.get('RUSHDB_SELF_HOSTED')) && forceUserSignUp) {
        await this.stripeService.createCustomer(login)
      }

      await this.workspaceService.removePendingInvite(workspaceId, login, transaction)
      isDevMode(() => Logger.log(`[Accept invite LOG] removed pending invite for ${login}`))

      return {
        userData: this.normalize(userNode),
        workspaceId
      }
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

  async createUserNode(properties: Omit<TUserProperties, 'id' | 'isEmail'>, transaction: Transaction) {
    let userSettings = {}
    const currentTime = getCurrentISO()
    const userId = uuidv7()
    const userPassword = await this.encryptionService.hash(properties.password)

    //  process settings only if they're present
    if (properties.settings) {
      userSettings = sanitizeSettings(properties.settings)
    }

    const isEmail = toBoolean(validateEmail(properties.login || ''))

    if (!isEmail) {
      isDevMode(() => Logger.error('[Create user ERROR]: Bad email provided'))
      throw new BadRequestException('Bad email data provided')
    }

    return await this.userRepository.model.createOne(
      {
        ...properties,
        isEmail,
        password: userPassword,
        confirmed: properties.confirmed ?? false,
        created: currentTime,
        id: userId,
        settings: JSON.stringify(userSettings)
      },
      { session: transaction }
    )
  }

  async update(
    id: string,
    userProperties: Partial<TUserProperties>,
    transaction: Transaction
  ): Promise<User | any> {
    const userNode = await this.findUserNodeById(id, transaction)

    if (!userNode && !transaction) {
      throw new BadRequestException(`User ${id} not found`)
    }

    const fieldsToUpdate = removeUndefinedKeys(userProperties)

    const updateField = async (key: string, value: TUserProperties[keyof TUserProperties]) => {
      if (userNode[key] !== value) {
        // hack to change an object in TUserProperties interface
        if (key === 'settings') {
          userNode[key] = sanitizeSettings(value)
        } else {
          userNode[key] = value
        }
      }
    }

    await Promise.all(
      Object.entries<TUserProperties[keyof TUserProperties]>(fieldsToUpdate).map(
        async ([key, value]) => await updateField(key, value)
      )
    )
    userNode['edited'] = getCurrentISO()

    await userNode.save()

    return this.normalize(userNode)
  }

  async resetUserPassword(
    newPasswordData: ResetPasswordAuthDto,
    tokenPayload: IDecodedResetToken,
    transaction: Transaction
  ): Promise<User> {
    const userToValidate = await this.find(newPasswordData.login, transaction).then((user) => user?.toJson())

    if (userToValidate.login !== tokenPayload.login || userToValidate.id !== tokenPayload.id) {
      throw new BadRequestException('Confirmation data malformed')
    }

    const { password: newPassword } = newPasswordData
    const newPasswordEncrypted = await this.encryptionService.hash(newPassword)

    return this.update(
      userToValidate.id,
      {
        password: newPasswordEncrypted
      },
      transaction
    )
  }

  // async setUserAsInactive(id: string, neogmaTransaction?: Transaction): Promise<boolean> {
  //     const transaction = neogmaTransaction;
  //
  //     const currentTime = getCurrentISO();
  //
  //     try {
  //         await this.update(
  //             id,
  //             {
  //                 status: USER_STATUS_DELETED,
  //                 deletedDate: currentTime,
  //             },
  //             transaction
  //         );
  //
  //         return true;
  //     } catch (e) {
  //         return false;
  //     }
  // }

  async hasMinimalAccessLevel({
    userId,
    targetId,
    targetType = 'workspace',
    accessLevel,
    transaction
  }: {
    userId: string
    targetId: string
    targetType?: 'project' | 'workspace'
    accessLevel?: TUserRoles
    transaction: Transaction
  }): Promise<boolean> {
    const queryRunner = this.neogmaService.createRunner()

    const queryBuilder = new QueryBuilder()

    const targetPart =
      targetType === 'workspace' ?
        `-[rel:${RUSHDB_RELATION_MEMBER_OF}]->(:${RUSHDB_LABEL_WORKSPACE} { id: $targetId })`
      : `-[rel:${RUSHDB_RELATION_HAS_ACCESS}]->(:${RUSHDB_LABEL_PROJECT} { id: $targetId }) `

    queryBuilder
      .append(`MATCH (:${RUSHDB_LABEL_USER} { id: $userId })${targetPart}`)
      .append(`RETURN rel.role as accessRole`)

    const result = await queryRunner.run(
      queryBuilder.build(),
      {
        targetId,
        userId
      },
      transaction
    )

    if (!result.records[0]) {
      return false
    }

    const currentRole: TUserRoles = result.records[0].get('accessRole')
    const requiredWeight = USER_ROLE_WEIGHT[accessLevel] ?? 0
    const currentWeight = USER_ROLE_WEIGHT[currentRole] ?? 0

    return currentWeight >= requiredWeight
  }

  async delete({ userId, transaction }: { userId: string; transaction: Transaction }) {
    try {
      const userNode = await this.findUserNodeById(userId, transaction)
      if (userNode) {
        const relatedWorkspaces = await userNode.findRelationships({
          alias: 'Workspaces',
          where: {
            relationship: {
              role: USER_ROLE_OWNER
            },
            target: {}
          },
          session: transaction
        })

        await Promise.all([
          ...relatedWorkspaces.map(
            async (rws) => await this.workspaceService.deleteWorkspace(rws.target.dataValues.id, transaction)
          ),
          userNode.delete({ detach: true })
        ])
      }
    } catch (e) {
      await transaction.rollback()
    }

    return true
  }

  async linkUser(id: string, projectId: string, transaction: Transaction): Promise<boolean> {
    const currentTime = getCurrentISO()

    try {
      const projectNode = await this.projectService.getProjectNode(projectId, transaction)

      if (!projectNode) {
        return false
      }
    } catch (e) {
      isDevMode(() =>
        Logger.warn(`[Link user to the project WARN]: Incorrect project id provided ${projectId}`, e)
      )

      return false
    }

    try {
      await this.projectService.linkUserToProject(id, projectId, currentTime, transaction)

      isDevMode(() =>
        Logger.log(`[Link user ${id} to the project LOG]: User linked to the project ${projectId}`)
      )
    } catch (e) {
      isDevMode(() => Logger.error('[Link user to the project ERROR]: Error while linking user', e))
      throw new BadRequestException('Cant link user to project')
    }

    return true
  }

  async getUserWorkspaceRole(login: string, workspaceId: string, transaction: Transaction) {
    return await this.workspaceService.getUserRoleInWorkspace(login, workspaceId, transaction)
  }
}
