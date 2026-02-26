import {
  BadRequestException,
  ForbiddenException,
  forwardRef,
  Inject,
  Injectable,
  Logger
} from '@nestjs/common'
import { InternalServerErrorException } from '@nestjs/common/exceptions/internal-server-error.exception'
import { ConfigService } from '@nestjs/config'
import { Transaction } from 'neo4j-driver'
import { uuidv7 } from 'uuidv7'

import { getCurrentISO } from '@/common/utils/getCurrentISO'
import { isDevMode } from '@/common/utils/isDevMode'
import { toBoolean } from '@/common/utils/toBolean'
import { BillingClientService } from '@/core/billing-client/billing-client.service'
import { removeUndefinedKeys } from '@/core/property/property.utils'
import { MailService } from '@/dashboard/mail/mail.service'
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
import { TWorkspaceInstance, TWorkspaceProperties } from '@/dashboard/workspace/model/workspace.interface'
import { WorkspaceRepository } from '@/dashboard/workspace/model/workspace.repository'
import { WorkspaceQueryService } from '@/dashboard/workspace/workspace-query.service'
import {
  TExtendedWorkspaceProperties,
  TNormalizedPendingInvite,
  TWorkspaceInvitation,
  TWorkSpaceInviteToken
} from '@/dashboard/workspace/workspace.types'
import { NeogmaService } from '@/database/neogma/neogma.service'

import * as crypto from 'node:crypto'

/*
 * Create Workspace --> Attach user that called this endpoint
 * Create Project in this Org
 *
 * Add user to project
 * Remove user to project
 * Change user role in project
 *
 * Edit workspace
 * Delete workspace
 * */

@Injectable()
export class WorkspaceService {
  private readonly logger = new Logger(WorkspaceService.name)

  constructor(
    private readonly configService: ConfigService,
    private readonly neogmaService: NeogmaService,
    private readonly workspaceRepository: WorkspaceRepository,
    private readonly workspaceQueryService: WorkspaceQueryService,
    private readonly billingClientService: BillingClientService,
    @Inject(forwardRef(() => ProjectService))
    private readonly projectService: ProjectService,
    @Inject(forwardRef(() => UserRepository))
    private readonly userRepository: UserRepository,
    @Inject(forwardRef(() => UserService))
    private readonly userService: UserService,
    @Inject(forwardRef(() => MailService))
    private readonly mailService: MailService
  ) {}

  normalize(node: TWorkspaceInstance) {
    return new Workspace(node)
  }
  async getWorkspaceInstance(id: string, transaction: Transaction): Promise<TWorkspaceInstance> {
    return await this.workspaceRepository.model.findOne({
      where: { id },
      throwIfNotFound: false,
      session: transaction
    })
  }

  async getWorkspace(id: string, transaction: Transaction): Promise<Workspace> {
    const workspaceInstance = await this.getWorkspaceInstance(id, transaction)
    const workspace = this.normalize(workspaceInstance)

    // Enrich with billing data if not self-hosted
    if (!toBoolean(this.configService.get('RUSHDB_SELF_HOSTED'))) {
      const workspaceJson = workspace.toJson()
      const enriched = await this.enrichWithBillingData(workspaceJson)
      return { ...workspace, toJson: () => enriched, getProperties: () => enriched } as Workspace
    }

    return workspace
  }

  async findUserBillingWorkspace(userEmail: string, transaction: Transaction): Promise<string> {
    const related = await this.userRepository.model.findRelationships({
      alias: 'Workspaces',
      where: {
        source: {
          login: userEmail
        },
        relationship: {
          role: USER_ROLE_OWNER
        }
      },
      session: transaction
    })

    const result = related.map(({ target }) => this.normalize(target).toJson())

    // @TODO: Improve for multiple workspaces owner support
    return result[0].id
  }

  async getWorkspaceNode(id: string, transaction: Transaction) {
    const queryRunner = this.neogmaService.createRunner()

    const workspace = await this.neogmaService
      .createBuilder()
      .match({
        model: this.workspaceRepository.model,
        where: { id },
        identifier: 'i'
      })
      .return('i')
      .run(queryRunner, transaction)

    const workspaceRecord = workspace.records[0]?.get('i')

    if (!workspaceRecord) {
      return
    }

    return this.workspaceRepository.model.buildFromRecord(workspaceRecord)
  }

  async getWorkspacesList(userId: string, transaction: Transaction): Promise<TExtendedWorkspaceProperties[]> {
    const related = await this.userRepository.model.findRelationships({
      alias: 'Workspaces',
      where: {
        source: {
          id: userId
        }
      },
      session: transaction
    })

    const workspaces = related.map(({ target, relationship }) => {
      return {
        ...this.normalize(target).toJson(),
        role: relationship.role
      }
    })

    // Enrich with billing data if not self-hosted
    if (!toBoolean(this.configService.get('RUSHDB_SELF_HOSTED'))) {
      return Promise.all(workspaces.map((ws) => this.enrichWithBillingData(ws)))
    }

    return workspaces
  }

  async getWorkspaceByProject(projectId: string, transaction: Transaction) {
    const related = await this.workspaceRepository.model.findRelationships({
      alias: 'Projects',
      where: {
        target: {
          id: projectId
        }
      },
      session: transaction
    })

    return related.map(({ source }) => source)?.[0]
  }

  async getAccumulatedWorkspaceStats(workspaceInstance: TWorkspaceInstance, transaction: Transaction) {
    const related = await workspaceInstance.findRelationships({
      alias: 'Projects',
      session: transaction
    })

    return related.reduce(
      (acc, { target: project }) => {
        try {
          const { stats } = project

          if (!stats) {
            return {
              records: acc.records,
              properties: acc.properties
            }
          }

          const { records, properties } = JSON.parse(stats) as TProjectStats
          return {
            records: acc.records + (records ?? 0),
            properties: acc.properties + (properties ?? 0)
          }
        } catch (error) {
          return acc
        }
      },
      { records: 0, properties: 0 }
    )
  }

  async createWorkspace(
    { name }: CreateWorkspaceDto,
    userId: string,
    transaction: Transaction
  ): Promise<Workspace> {
    const workspaceId = uuidv7()
    const workspaceNode = await this.workspaceRepository.model.createOne(
      {
        name,
        created: getCurrentISO(),
        id: workspaceId
      },
      { session: transaction }
    )

    await workspaceNode.relateTo({
      alias: 'Users',
      where: { id: userId },
      properties: { Since: workspaceNode.created, Role: USER_ROLE_OWNER },
      session: transaction
    })
    // Create customer in billing service — include owner email for notifications
    const ownerNode = await this.userRepository.model.findOne({
      where: { id: userId },
      throwIfNotFound: false,
      session: transaction
    })
    const ownerEmail = ownerNode?.login ?? null
    await this.billingClientService.createCustomer(workspaceId, 'free', ownerEmail)

    return this.normalize(workspaceNode)
  }

  async attachUserToWorkspace(
    workspaceId: string,
    userId: string,
    preferredRole: TUserRoles,
    transaction: Transaction
  ): Promise<void> {
    await transaction.run(this.workspaceQueryService.attachUserToWorkspaceQuery(), {
      workspaceId,
      userId,
      since: getCurrentISO(),
      role: preferredRole
    })

    isDevMode(() =>
      Logger.log(`[Link user ${userId} to the workspace LOG]: User linked to the workspace ${workspaceId}`)
    )
  }

  async patchWorkspace(
    id: string,
    workspaceProperties: Partial<TWorkspaceProperties>,
    transaction: Transaction
  ): Promise<Workspace> {
    const workspaceNode = await this.getWorkspaceNode(id, transaction)

    if (!workspaceNode && !transaction) {
      throw new BadRequestException(`Workspace ${id} not found`)
    }

    const fieldsToUpdate = removeUndefinedKeys(workspaceProperties)

    Object.entries<TWorkspaceProperties[keyof TWorkspaceProperties]>(fieldsToUpdate).map(([key, value]) => {
      if (workspaceNode[key] !== value) {
        workspaceNode[key] = value
      }
    })

    workspaceNode['edited'] = getCurrentISO()

    await workspaceNode.save()

    return this.normalize(workspaceNode)
  }

  async dropWorkspaceSubscription(id: string, transaction: Transaction): Promise<Workspace> {
    const workspaceNode = await this.getWorkspaceNode(id, transaction)
    workspaceNode['isSubscriptionCancelled'] = true
    workspaceNode['edited'] = getCurrentISO()

    await workspaceNode.save()

    return this.normalize(workspaceNode)
  }

  // @TODO: Ask to delete all project or transfer workspace ownership to someone else
  // @TODO: Delete users from workspace
  async deleteWorkspace(id: string, transaction: Transaction): Promise<{ message: string }> {
    const workspace = await this.workspaceRepository.model.findOne({
      where: { id },
      session: transaction
    })

    const projects = await this.workspaceRepository.model.findRelationships({
      alias: 'Projects',
      where: {
        source: { id: id }
      },
      session: transaction
    })

    const projectsToDelete = projects
      .map(async (project) =>
        project.target.id ?
          await this.projectService.deleteProject(project.target.id, transaction, true)
        : undefined
      )
      .filter(Boolean)

    await workspace.delete({ detach: true, session: transaction })
    await Promise.all(projectsToDelete)

    // Delete customer from billing service
    await this.billingClientService.deleteCustomer(id)

    return {
      message: `Workspace ${id} successfully deleted`
    }
  }

  encryptMemberToken(payload: TWorkSpaceInviteToken) {
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

  async inviteMember(payload: TWorkspaceInvitation, transaction: Transaction): Promise<{ message: string }> {
    const isEmail = toBoolean(validateEmail(payload.email || ''))

    if (!payload.workspaceId || !payload.email || !isEmail) {
      isDevMode(() => Logger.error('[Invite member ERROR]: No required data provided'))
      throw new BadRequestException('No required data provided')
    }

    const { workspaceId, projectIds, email, ...rest } = payload

    const token = this.encryptMemberToken({
      workspaceId,
      projectIds,
      email
    })

    try {
      const pendingList = await this.getPendingInvites(workspaceId, transaction)
      const isInviteAlreadySent = pendingList.find(({ email }) => email === payload.email)

      if (!isInviteAlreadySent) {
        pendingList.push({
          email: payload.email,
          createdAt: getCurrentISO()
        })

        await transaction.run(this.workspaceQueryService.setPendingInvitesQuery(), {
          workspaceId: payload.workspaceId,
          invites: JSON.stringify(pendingList)
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

    return {
      message: `Invite for ${email} successfully sent`
    }
  }

  async getPendingInvites(
    workspaceId: string,
    transaction: Transaction
  ): Promise<TNormalizedPendingInvite[]> {
    const currentPendingInvites = await transaction.run(this.workspaceQueryService.getPendingInvitesQuery(), {
      workspaceId
    })
    const result = currentPendingInvites.records[0]?.get('invites')

    if (!result) {
      return []
    }

    try {
      return JSON.parse(result)
    } catch {
      return []
    }
  }

  async removePendingInvite(
    workspaceId: string,
    email: string,
    transaction: Transaction
  ): Promise<{ message: string }> {
    const currentPendingInvites = await this.getPendingInvites(workspaceId, transaction)

    const filtered = currentPendingInvites.filter((item) => item.email !== email)

    await transaction.run(this.workspaceQueryService.setPendingInvitesQuery(), {
      workspaceId,
      invites: JSON.stringify(filtered)
    })

    isDevMode(() => Logger.log(`[Invite member LOG]: Invitation revoke from the ${email}`))

    return {
      message: 'Invitation revoked'
    }
  }

  async recomputeProjectsAccessList(id: string, payload: RecomputeAccessListDto, transaction: Transaction) {
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
    transaction: Transaction
  ): Promise<Record<string, string[]>> {
    const result = await transaction.run(this.workspaceQueryService.getWorkspaceAccessListQuery(), {
      workspaceId,
      role: USER_ROLE_EDITOR
    })

    const accessMap: Record<string, string[]> = {}

    for (const record of result.records) {
      const projectId = record.get('projectId')
      accessMap[projectId] = record.get('userIds') || []
    }

    return accessMap
  }

  async getUserList(workspaceId: string, transaction: Transaction): Promise<TShortUserDataWithRole[]> {
    const result = await transaction.run(this.workspaceQueryService.getWorkspaceUserListQuery(), {
      workspaceId
    })

    return result.records.map((record) => ({
      id: record.get('id'),
      login: record.get('login'),
      role: record.get('role')
    })) as TShortUserDataWithRole[]
  }

  async revokeAccessList(workspaceId: string, userIds: string[], transaction: Transaction) {
    for (const userId of userIds) {
      isDevMode(() => Logger.log(`[Revoke access LOG]: Check other workspaces for user ${userId}`))
      const countsResult = await transaction.run(
        this.workspaceQueryService.getUserRoleCountsOutsideWorkspaceQuery(),
        { userId, workspaceId }
      )

      const record = countsResult.records[0]
      const ownerOther =
        record.get('ownerOther').toNumber ? record.get('ownerOther').toNumber() : record.get('ownerOther')
      const developerOther =
        record.get('developerOther').toNumber ?
          record.get('developerOther').toNumber()
        : record.get('developerOther')

      if (ownerOther > 0 || developerOther > 0) {
        isDevMode(() => Logger.log(`[Revoke access LOG]: Remove ws relation for user ${userId}`))
        await transaction.run(this.workspaceQueryService.getRemoveWorkspaceRelationQuery(), {
          userId,
          workspaceId
        })

        isDevMode(() => Logger.log(`[Revoke access LOG]: Remove project relation for user ${userId}`))
        await transaction.run(this.workspaceQueryService.getRemoveProjectRelationsQuery(), {
          userId,
          workspaceId
        })
      } else {
        isDevMode(() =>
          Logger.log(`[Revoke access LOG]: No other entities found for user ${userId}, delete user data`)
        )
        await this.userService.delete({ userId, transaction })
      }
    }

    return { message: 'Access revoked where appropriate' }
  }

  async leaveWorkspace(workspaceId: string, userId: string, transaction: Transaction): Promise<void> {
    isDevMode(() => Logger.log(`[Revoke access LOG]: Remove ws relation for user ${userId}`))
    await transaction.run(this.workspaceQueryService.getRemoveWorkspaceRelationQuery(), {
      userId,
      workspaceId
    })

    isDevMode(() => Logger.log(`[Revoke access LOG]: Remove project relation for user ${userId}`))
    await transaction.run(this.workspaceQueryService.getRemoveProjectRelationsQuery(), {
      userId,
      workspaceId
    })
  }

  async getUserRoleInWorkspace(
    login: string,
    workspaceId: string,
    transaction: Transaction
  ): Promise<TUserRoles> {
    const result = await transaction.run(this.workspaceQueryService.getUserWorkspaceRoleQuery(), {
      login,
      workspaceId
    })

    const rec = result.records[0]

    if (!rec) {
      isDevMode(() => Logger.error('[Get User Role ERROR]: No role found'))

      throw new ForbiddenException('No user role for workspace found')
    }

    return rec.get('role') as TUserRoles
  }

  async getUserRoleInWorkspaceById(
    id: string,
    workspaceId: string,
    transaction: Transaction
  ): Promise<TUserRoles> {
    const result = await transaction.run(this.workspaceQueryService.getUserWorkspaceRoleByIdQuery(), {
      id,
      workspaceId
    })

    const rec = result.records[0]

    if (!rec) {
      isDevMode(() => Logger.error('[Get User Role ERROR]: No role found'))

      throw new ForbiddenException('No user role for workspace found')
    }

    return rec.get('role') as TUserRoles
  }

  /**
   * Enrich workspace properties with billing data from the billing service.
   * Adds planId, validTill, and isSubscriptionCancelled fields.
   *
   * @param workspace - Workspace properties
  /**
   * Enrich workspace with billing data from billing service.
   *
   * Injects billing-related fields that are not stored in platform database:
   * - planId: current subscription plan
   * - validTill: subscription expiration (if canceled)
   * - isSubscriptionCancelled: whether subscription is canceled
   * - projectLimit: max projects allowed (from billing service)
   * - userLimit: max users allowed (from billing service)
   *
   * @param workspace - Workspace properties from Neo4j
   * @returns Enriched workspace properties with billing data
   */
  private async enrichWithBillingData<T extends TWorkspaceProperties>(workspace: T): Promise<T> {
    try {
      const customer = await this.billingClientService.getCustomer(workspace.id)

      if (!customer) {
        // Remove legacy limits field if it exists
        const { limits, ...workspaceWithoutLimits } = workspace as any
        return {
          ...workspaceWithoutLimits,
          planId: 'free',
          validTill: undefined,
          isSubscriptionCancelled: false,
          projectLimit: 2, // Free plan default
          userLimit: 1 // Free plan default
        } as T
      }

      // Map billing service plan names to UI plan IDs
      const planIdMap: Record<string, string> = {
        free: 'free',
        pro: 'pro',
        scale: 'scale',
        enterprise: 'enterprise'
      }

      // Only set validTill if subscription is canceled (i.e., will expire)
      // For active subscriptions or free plans, leave as undefined
      const validTill = customer.subscriptionStatus === 'canceled' ? customer.billingPeriodStart : undefined

      // Remove legacy limits field if it exists
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

      // Remove legacy limits field if it exists
      const { limits, ...workspaceWithoutLimits } = workspace as any

      // Return workspace with free plan on error
      return {
        ...workspaceWithoutLimits,
        planId: 'free',
        validTill: undefined,
        isSubscriptionCancelled: false,
        projectLimit: 2, // Free plan default
        userLimit: 1 // Free plan default
      } as T
    }
  }
}
