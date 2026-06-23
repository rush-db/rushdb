import {
  BadRequestException,
  ForbiddenException,
  forwardRef,
  Inject,
  Injectable,
  InternalServerErrorException,
  Logger
} from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { Transaction } from 'neo4j-driver'
import { uuidv7 } from 'uuidv7'

import { getCurrentISO } from '@/common/utils/getCurrentISO'
import { isDevMode } from '@/common/utils/isDevMode'
import { toBoolean } from '@/common/utils/toBolean'
import { BillingClientService } from '@/core/billing-client/billing-client.service'
import { BILLING_ACCOUNT_PORT, BillingAccountPort } from '@/core/billing-policy/billing-account.port'
import { removeUndefinedKeys } from '@/core/property/property.utils'
import { MailService } from '@/dashboard/mail/mail.service'
import { OAuthRepository } from '@/dashboard/mcp-oauth/model/oauth.repository'
import { ProjectRepository } from '@/dashboard/project/model/project.repository'
import { ProjectService } from '@/dashboard/project/project.service'
import { TProjectStats } from '@/dashboard/project/project.types'
import { TShortUserDataWithRole } from '@/dashboard/user/interfaces/authenticated-user.interface'
import { USER_ROLE_EDITOR, USER_ROLE_OWNER } from '@/dashboard/user/interfaces/user.constants'
import { TUserRoles } from '@/dashboard/user/model/user.interface'
import { UserRepository } from '@/dashboard/user/model/user.repository'
import { UserService } from '@/dashboard/user/user.service'
import { validateEmail } from '@/dashboard/user/user.utils'
import { CreateWorkspaceDto } from '@/dashboard/workspace/dto/create-workspace.dto'
import { RecomputeAccessListDto } from '@/dashboard/workspace/dto/recompute-access-list.dto'
import { Workspace } from '@/dashboard/workspace/entity/workspace.entity'
import { TWorkspaceProperties } from '@/dashboard/workspace/model/workspace.interface'
import { WorkspaceRepository } from '@/dashboard/workspace/model/workspace.repository'
import {
  TExtendedWorkspaceProperties,
  TNormalizedPendingInvite,
  TWorkspaceInvitation,
  TWorkSpaceInviteToken
} from '@/dashboard/workspace/workspace.types'

import * as crypto from 'node:crypto'

import type { WorkspaceRow } from '@/database/sql/schema/types'

@Injectable()
export class WorkspaceService {
  private readonly logger = new Logger(WorkspaceService.name)

  constructor(
    private readonly configService: ConfigService,
    private readonly workspaceRepository: WorkspaceRepository,
    @Inject(BILLING_ACCOUNT_PORT)
    private readonly billingAccountService: BillingAccountPort,
    private readonly billingClientService: BillingClientService,
    private readonly oauthRepository: OAuthRepository,
    @Inject(forwardRef(() => ProjectService))
    private readonly projectService: ProjectService,
    @Inject(forwardRef(() => ProjectRepository))
    private readonly projectRepository: ProjectRepository,
    @Inject(forwardRef(() => UserRepository))
    private readonly userRepository: UserRepository,
    @Inject(forwardRef(() => UserService))
    private readonly userService: UserService,
    @Inject(forwardRef(() => MailService))
    private readonly mailService: MailService
  ) {}

  normalize(row: WorkspaceRow): Workspace {
    return new Workspace(row)
  }

  async getWorkspace(id: string, _transaction?: Transaction): Promise<Workspace> {
    const row = await this.workspaceRepository.findById(id)
    const workspace = this.normalize(row)

    if (!toBoolean(this.configService.get('RUSHDB_SELF_HOSTED'))) {
      const workspaceJson = workspace.toJson()
      const enriched = await this.enrichWithBillingData(workspaceJson)
      return { ...workspace, toJson: () => enriched, getProperties: () => enriched } as Workspace
    }

    return workspace
  }

  /** Alias used by legacy call sites (e.g. workspace.controller.ts invite flow) */
  async getWorkspaceInstance(id: string, _transaction?: Transaction): Promise<WorkspaceRow> {
    return this.workspaceRepository.findById(id)
  }

  async findUserBillingWorkspace(userEmail: string, _transaction?: Transaction): Promise<string> {
    const userRow = await this.userRepository.findByLogin(userEmail)
    if (!userRow) {
      throw new BadRequestException('User not found')
    }

    const workspaces = await this.userRepository.findWorkspacesForUser(userRow.id)
    const ownerEntry = workspaces.find((w) => w.role === USER_ROLE_OWNER)
    return ownerEntry?.workspace?.id
  }

  async getWorkspacesList(
    userId: string,
    _transaction?: Transaction
  ): Promise<TExtendedWorkspaceProperties[]> {
    const rows = await this.userRepository.findWorkspacesForUser(userId)

    const workspaces = rows.map(({ workspace, role }) => ({
      ...this.normalize(workspace).toJson(),
      role: role as TUserRoles
    }))

    if (!toBoolean(this.configService.get('RUSHDB_SELF_HOSTED'))) {
      return Promise.all(workspaces.map((ws) => this.enrichWithBillingData(ws)))
    }

    return workspaces
  }

  async getWorkspaceByProject(
    projectId: string,
    _transaction?: Transaction
  ): Promise<WorkspaceRow | undefined> {
    return this.workspaceRepository.findByProjectId(projectId)
  }

  async getAccumulatedWorkspaceStats(workspaceId: string, _transaction?: Transaction) {
    const projects = await this.projectRepository.findByWorkspaceId(workspaceId)

    return projects.reduce(
      (acc, project) => {
        try {
          const { stats } = project as any
          if (!stats) {
            return acc
          }
          const { records, properties } = JSON.parse(stats) as TProjectStats
          return {
            records: acc.records + (records ?? 0),
            properties: acc.properties + (properties ?? 0)
          }
        } catch {
          return acc
        }
      },
      { records: 0, properties: 0 }
    )
  }

  async createWorkspace(
    { name }: CreateWorkspaceDto,
    userId: string,
    _transaction?: Transaction
  ): Promise<Workspace> {
    const workspaceId = uuidv7()
    const created = getCurrentISO()

    const workspaceRow = await this.workspaceRepository.create({
      id: workspaceId,
      name,
      created,
      edited: null
    })

    await this.workspaceRepository.addMember({
      workspaceId,
      userId,
      role: USER_ROLE_OWNER,
      since: created
    })

    const ownerNode = await this.userRepository.findById(userId)
    const ownerEmail = ownerNode?.login ?? null
    await this.billingAccountService.createWorkspaceCustomer(workspaceId, ownerEmail)

    return this.normalize(workspaceRow)
  }

  async attachUserToWorkspace(
    workspaceId: string,
    userId: string,
    preferredRole: TUserRoles,
    _transaction?: Transaction
  ): Promise<void> {
    await this.workspaceRepository.addMember({
      workspaceId,
      userId,
      role: preferredRole,
      since: getCurrentISO()
    })

    isDevMode(() =>
      Logger.log(`[Link user ${userId} to the workspace LOG]: User linked to the workspace ${workspaceId}`)
    )

    // Emit seat meter event to billing service
    const members = await this.workspaceRepository.getMembers(workspaceId)
    await this.billingClientService.emitSeatMeterEvent(workspaceId, members.length)
  }

  async changeMemberRole(
    workspaceId: string,
    userId: string,
    role: TUserRoles,
    requesterId?: string,
    _transaction?: Transaction
  ): Promise<{ message: string }> {
    // A user cannot change their own workspace role (prevents an owner from
    // accidentally downgrading themselves and locking the workspace).
    if (requesterId && requesterId === userId) {
      throw new BadRequestException('You cannot change your own role')
    }

    const member = await this.workspaceRepository.getMember(workspaceId, userId)
    if (!member) {
      throw new BadRequestException('User is not a member of this workspace')
    }

    // Prevent demoting the last owner — a workspace must always have at least one owner.
    if (member.role === USER_ROLE_OWNER && role !== USER_ROLE_OWNER) {
      const members = await this.workspaceRepository.getMembers(workspaceId)
      const ownerCount = members.filter((m) => m.role === USER_ROLE_OWNER).length
      if (ownerCount <= 1) {
        throw new BadRequestException('Cannot demote the last owner of the workspace')
      }
    }

    await this.workspaceRepository.updateMemberRole(workspaceId, userId, role)
    isDevMode(() =>
      Logger.log(`[Change member role LOG]: User ${userId} role set to ${role} in workspace ${workspaceId}`)
    )
    return { message: 'Member role updated' }
  }

  async attachUserToWorkspaceIfAbsent(
    workspaceId: string,
    userId: string,
    preferredRole: TUserRoles,
    transaction?: Transaction
  ): Promise<void> {
    const member = await this.workspaceRepository.getMember(workspaceId, userId)
    if (!member) {
      await this.attachUserToWorkspace(workspaceId, userId, preferredRole, transaction)
    }
  }

  async patchWorkspace(
    id: string,
    workspaceProperties: Partial<TWorkspaceProperties>,
    _transaction?: Transaction
  ): Promise<Workspace> {
    const fieldsToUpdate = removeUndefinedKeys(workspaceProperties) as any
    fieldsToUpdate.edited = getCurrentISO()

    const updated = await this.workspaceRepository.update(id, fieldsToUpdate)
    if (!updated) {
      throw new BadRequestException(`Workspace ${id} not found`)
    }
    return this.normalize(updated)
  }

  async dropWorkspaceSubscription(id: string, _transaction?: Transaction): Promise<Workspace> {
    const updated = await this.workspaceRepository.update(id, {
      edited: getCurrentISO()
    })
    return this.normalize(updated)
  }

  async deleteWorkspace(id: string, _transaction?: Transaction): Promise<{ message: string }> {
    const projects = await this.projectRepository.findByWorkspaceId(id)
    const projectIds = projects.map((p) => p.id).filter(Boolean) as string[]

    await Promise.all(projectIds.map((pid) => this.projectService.deleteProject(pid, _transaction, true)))

    // Clean up OAuth consents that pointed at now-deleted projects, then
    // remove any oauth_clients that are no longer referenced by any consent.
    if (projectIds.length) {
      await this.oauthRepository.deleteConsentsByProjectIds(projectIds)
    }
    await this.oauthRepository.deleteOrphanedClients()

    await this.workspaceRepository.delete(id)
    await this.billingAccountService.deleteWorkspaceCustomer(id)

    return { message: `Workspace ${id} successfully deleted` }
  }

  encryptMemberToken(payload: TWorkSpaceInviteToken): string {
    const encryptionKey = this.configService.get('RUSHDB_AES_256_ENCRYPTION_KEY')
    const iv = crypto.randomBytes(16)
    const invitationString = JSON.stringify({
      workspaceId: payload.workspaceId,
      projectIds: payload.projectIds,
      email: payload.email
    })
    const cipher = crypto.createCipheriv('aes-256-cbc', encryptionKey, iv)
    return iv.toString('hex') + cipher.update(invitationString, 'utf8', 'base64') + cipher.final('base64')
  }

  async inviteMember(
    payload: TWorkspaceInvitation,
    _transaction?: Transaction
  ): Promise<{ message: string }> {
    const isEmail = toBoolean(validateEmail(payload.email || ''))

    if (!payload.workspaceId || !payload.email || !isEmail) {
      isDevMode(() => Logger.error('[Invite member ERROR]: No required data provided'))
      throw new BadRequestException('No required data provided')
    }

    const { workspaceId, projectIds, email, ...rest } = payload

    const token = this.encryptMemberToken({ workspaceId, projectIds, email })

    try {
      const pendingList = await this.getPendingInvites(workspaceId)
      const isInviteAlreadySent = pendingList.find((inv) => inv.email === payload.email)

      if (!isInviteAlreadySent) {
        await this.workspaceRepository.addInvite({
          id: uuidv7(),
          workspaceId,
          email,
          createdAt: getCurrentISO()
        })
        isDevMode(() => Logger.log(`[Invite member LOG]: Invitation ${email} added for user workspace`))
      }
    } catch (e) {
      isDevMode(() => Logger.error('[Invite member ERROR]: Error updating invitation list', e))
      throw new InternalServerErrorException('Error updating invitation list')
    }

    try {
      await this.mailService.sendUserInvite({
        login: email,
        token,
        senderName: rest.senderEmail,
        workspaceName: rest.workspaceName
      })
      isDevMode(() => Logger.log(`[Invite member LOG]: Invitation sent to the ${email}`))
    } catch (e) {
      isDevMode(() => Logger.error('[Invite member ERROR]: Error sending an email', e))
      throw new InternalServerErrorException('Error while sending email')
    }

    return { message: `Invite for ${email} successfully sent` }
  }

  async getPendingInvites(
    workspaceId: string,
    _transaction?: Transaction
  ): Promise<TNormalizedPendingInvite[]> {
    const invites = await this.workspaceRepository.getInvites(workspaceId)
    return invites.map((inv) => ({ email: inv.email, createdAt: inv.createdAt }))
  }

  async removePendingInvite(
    workspaceId: string,
    email: string,
    _transaction?: Transaction
  ): Promise<{ message: string }> {
    await this.workspaceRepository.removeInvite(workspaceId, email)
    isDevMode(() => Logger.log(`[Invite member LOG]: Invitation revoke from the ${email}`))
    return { message: 'Invitation revoked' }
  }

  async recomputeProjectsAccessList(id: string, payload: RecomputeAccessListDto, transaction?: Transaction) {
    for (const [projectId, userIds] of Object.entries(payload)) {
      await this.projectService.processUserAccess({
        projectId,
        userIdsToVerify: userIds,
        transaction
      })
    }
    return { message: 'Access lists recomputed' }
  }

  async getAccessListByProjects(
    workspaceId: string,
    _transaction?: Transaction
  ): Promise<Record<string, string[]>> {
    const projects = await this.projectRepository.findByWorkspaceId(workspaceId)
    const accessMap: Record<string, string[]> = {}

    await Promise.all(
      projects.map(async (project) => {
        const list = await this.projectRepository.getAccessListByProjectId(project.id)
        accessMap[project.id] = list.filter((a) => a.role === USER_ROLE_EDITOR).map((a) => a.userId)
      })
    )

    return accessMap
  }

  async getUserList(workspaceId: string, _transaction?: Transaction): Promise<TShortUserDataWithRole[]> {
    const members = await this.workspaceRepository.getMembers(workspaceId)
    return members.map((m) => ({ id: m.id, login: m.login, role: m.role as TUserRoles }))
  }

  async revokeAccessList(workspaceId: string, userIds: string[], _transaction?: Transaction) {
    for (const userId of userIds) {
      isDevMode(() => Logger.log(`[Revoke access LOG]: Check other workspaces for user ${userId}`))
      const { ownerOther, developerOther } = await this.workspaceRepository.getMembersWithOwnerCount(
        workspaceId,
        userId
      )

      if (ownerOther > 0 || developerOther > 0) {
        isDevMode(() => Logger.log(`[Revoke access LOG]: Remove ws relation for user ${userId}`))
        await this.workspaceRepository.removeMember(workspaceId, userId)

        isDevMode(() => Logger.log(`[Revoke access LOG]: Remove project relation for user ${userId}`))
        await this.projectRepository.revokeAllProjectAccessForUserInWorkspace(userId, workspaceId)
      } else {
        isDevMode(() =>
          Logger.log(`[Revoke access LOG]: No other entities found for user ${userId}, delete user data`)
        )
        await this.userService.delete({ userId })
      }
    }

    // Emit seat meter event to billing service after all members removed
    const members = await this.workspaceRepository.getMembers(workspaceId)
    await this.billingClientService.emitSeatMeterEvent(workspaceId, members.length)

    return { message: 'Access revoked where appropriate' }
  }

  async leaveWorkspace(workspaceId: string, userId: string, _transaction?: Transaction): Promise<void> {
    isDevMode(() => Logger.log(`[Revoke access LOG]: Remove ws relation for user ${userId}`))
    await this.workspaceRepository.removeMember(workspaceId, userId)

    isDevMode(() => Logger.log(`[Revoke access LOG]: Remove project relation for user ${userId}`))
    await this.projectRepository.revokeAllProjectAccessForUserInWorkspace(userId, workspaceId)

    // Emit seat meter event to billing service
    const members = await this.workspaceRepository.getMembers(workspaceId)
    await this.billingClientService.emitSeatMeterEvent(workspaceId, members.length)
  }

  async getUserRoleInWorkspace(
    login: string,
    workspaceId: string,
    _transaction?: Transaction
  ): Promise<TUserRoles> {
    const userRow = await this.userRepository.findByLogin(login)
    if (!userRow) {
      isDevMode(() => Logger.error('[Get User Role ERROR]: User not found'))
      throw new ForbiddenException('No user role for workspace found')
    }

    const member = await this.workspaceRepository.getMember(workspaceId, userRow.id)
    if (!member) {
      isDevMode(() => Logger.error('[Get User Role ERROR]: No role found'))
      throw new ForbiddenException('No user role for workspace found')
    }

    return member.role as TUserRoles
  }

  async getUserRoleInWorkspaceById(
    id: string,
    workspaceId: string,
    _transaction?: Transaction
  ): Promise<TUserRoles> {
    const member = await this.workspaceRepository.getMember(workspaceId, id)
    if (!member) {
      isDevMode(() => Logger.error('[Get User Role ERROR]: No role found'))
      throw new ForbiddenException('No user role for workspace found')
    }
    return member.role as TUserRoles
  }

  private async enrichWithBillingData<T extends TWorkspaceProperties>(workspace: T): Promise<T> {
    try {
      const customer = await this.billingAccountService.getWorkspaceCustomer(workspace.id)

      if (!customer) {
        const { limits, ...workspaceWithoutLimits } = workspace as any
        return {
          ...workspaceWithoutLimits,
          planId: 'free',
          validTill: undefined,
          isSubscriptionCancelled: false,
          projectLimit: 2,
          userLimit: 1
        } as T
      }

      const planIdMap: Record<string, string> = {
        free: 'free',
        pro: 'pro',
        scale: 'scale',
        enterprise: 'enterprise'
      }

      const validTill = customer.subscriptionStatus === 'canceled' ? customer.billingPeriodStart : undefined
      const { limits, ...workspaceWithoutLimits } = workspace as any

      return {
        ...workspaceWithoutLimits,
        planId: planIdMap[customer.plan] || 'free',
        validTill,
        isSubscriptionCancelled: customer.subscriptionStatus === 'canceled',
        projectLimit: customer.projectLimit,
        userLimit: customer.userLimit
      } as T
    } catch (error) {
      this.logger.error(`Failed to enrich workspace ${workspace.id} with billing data: ${error.message}`)
      const { limits, ...workspaceWithoutLimits } = workspace as any
      return {
        ...workspaceWithoutLimits,
        planId: 'free',
        validTill: undefined,
        isSubscriptionCancelled: false,
        projectLimit: 2,
        userLimit: 1
      } as T
    }
  }
}
