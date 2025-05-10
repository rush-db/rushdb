import { BadRequestException, forwardRef, Inject, Injectable, Logger } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { Transaction } from 'neo4j-driver'
import { uuidv7 } from 'uuidv7'

import { getCurrentISO } from '@/common/utils/getCurrentISO'
import { toBoolean } from '@/common/utils/toBolean'
import { removeUndefinedKeys } from '@/core/property/property.utils'
import { ProjectService } from '@/dashboard/project/project.service'
import { TProjectStats } from '@/dashboard/project/project.types'
import { USER_ROLE_EDITOR, USER_ROLE_OWNER } from '@/dashboard/user/interfaces/user.constants'
import { UserRepository } from '@/dashboard/user/model/user.repository'
import { UserService } from '@/dashboard/user/user.service'
import { CreateWorkspaceDto } from '@/dashboard/workspace/dto/create-workspace.dto'
import { Workspace } from '@/dashboard/workspace/entity/workspace.entity'
import * as crypto from 'node:crypto'
import {
  TWorkspaceInstance,
  TWorkspaceLimits,
  TWorkspaceProperties
} from '@/dashboard/workspace/model/workspace.interface'
import { WorkspaceRepository } from '@/dashboard/workspace/model/workspace.repository'
import {
  WORKSPACE_LIMITS_DEFAULT,
  WORKSPACE_LIMITS_PRO,
  WORKSPACE_LIMITS_START,
  WORKSPACE_LIMITS_WHITE_LABEL
} from '@/dashboard/workspace/workspace.constants'
import { NeogmaService } from '@/database/neogma/neogma.service'
import { EConfigKeyByPlan } from '@/dashboard/billing/stripe/interfaces/stripe.constans'
import { TWorkspaceInvitation, TWorkSpaceInviteToken } from '@/dashboard/workspace/workspace.types'
import { isDevMode } from '@/common/utils/isDevMode'
import { MailService } from '@/dashboard/mail/mail.service'
import { InternalServerErrorException } from '@nestjs/common/exceptions/internal-server-error.exception'
import { TUserRoles } from '@/dashboard/user/model/user.interface'
import { RecomputeAccessListDto } from '@/dashboard/workspace/dto/recompute-access-list.dto'
import { WorkspaceQueryService } from '@/dashboard/workspace/workspace-query.service'
import { IUserClaims } from '@/dashboard/user/interfaces/user-claims.interface'

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
  constructor(
    private readonly configService: ConfigService,
    private readonly neogmaService: NeogmaService,
    private readonly workspaceRepository: WorkspaceRepository,
    private readonly workspaceQueryService: WorkspaceQueryService,
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

    return this.normalize(workspaceInstance)
  }

  async findUserBillingWorkspace(userId: string, transaction: Transaction): Promise<string> {
    const related = await this.userRepository.model.findRelationships({
      alias: 'Workspaces',
      where: {
        source: {
          id: userId
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

  async getWorkspacesList(userId: string, transaction: Transaction): Promise<TWorkspaceProperties[]> {
    const related = await this.userRepository.model.findRelationships({
      alias: 'Workspaces',
      where: {
        source: {
          id: userId
        }
      },
      session: transaction
    })

    return related.map(({ target }) => this.normalize(target).toJson())
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

  getLimitsByKey(key = ''): TWorkspaceLimits {
    if (toBoolean(this.configService.get('RUSHDB_SELF_HOSTED'))) {
      return WORKSPACE_LIMITS_WHITE_LABEL
    }

    if (!toBoolean(key)) {
      return WORKSPACE_LIMITS_DEFAULT
    }

    // @TODO: remove contract between billing service && workspace service
    switch (key) {
      case EConfigKeyByPlan.pro:
        return WORKSPACE_LIMITS_PRO
      case EConfigKeyByPlan.start:
        return WORKSPACE_LIMITS_START
      default:
        return WORKSPACE_LIMITS_DEFAULT
    }
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
        id: workspaceId,
        limits: JSON.stringify(this.getLimitsByKey())
      },
      { session: transaction }
    )

    await workspaceNode.relateTo({
      alias: 'Users',
      where: { id: userId },
      properties: { Since: workspaceNode.created, Role: USER_ROLE_OWNER },
      session: transaction
    })

    return this.normalize(workspaceNode)
  }

  async attachUserToWorkspace(
    workspaceId: string,
    userId: string,
    preferredRole: TUserRoles,
    transaction: Transaction
  ): Promise<Workspace> {
    const workspaceNode = await this.getWorkspaceNode(workspaceId, transaction)

    await workspaceNode.relateTo({
      alias: 'Users',
      where: { id: userId },
      properties: { Since: workspaceNode.created, Role: preferredRole },
      session: transaction
    })

    isDevMode(() =>
      Logger.log(`[Link user ${userId} to the workspace LOG]: User linked to the workspace ${workspaceId}`)
    )

    return this.normalize(workspaceNode)
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
          await this.projectService.deleteProject(project.target.id, transaction)
        : undefined
      )
      .filter(Boolean)

    await workspace.delete({ detach: true, session: transaction })
    await Promise.all(projectsToDelete)

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
      email: payload.email,
      isUserRegistered: payload.isUserRegistered
    })

    const cipher = crypto.createCipheriv('aes-256-cbc', encryptionKey, iv)

    return iv.toString('hex') + cipher.update(invitationString, 'utf8', 'base64') + cipher.final('base64')
  }

  async inviteMember(payload: TWorkspaceInvitation, transaction: Transaction): Promise<{ message: string }> {
    if (!payload.workspaceId || !payload.email) {
      isDevMode(() => Logger.error('[Invite member ERROR]: No required data provided'))
      throw new BadRequestException('No required data provided')
    }

    const { workspaceId, projectIds, email, ...rest } = payload

    let shouldOnlyNotify = false
    const userNode = await this.userService.find(email, transaction)

    if (userNode?.getId()) {
      isDevMode(() =>
        Logger.log(`[Invite member LOG]: Invitation will be send to the already registered user ${email}`)
      )
      shouldOnlyNotify = true
    }

    const token = this.encryptMemberToken({
      workspaceId,
      projectIds,
      email,
      isUserRegistered: shouldOnlyNotify
    })

    try {
      await this.mailService.sendUserInvite(
        email,
        token,
        shouldOnlyNotify,
        rest.senderEmail,
        rest.workspaceName
      )
      isDevMode(() => Logger.log(`[Invite member LOG]: Invitation sent to the ${email}`))
    } catch (e) {
      isDevMode(() => Logger.error('[Invite member ERROR]: Error sending an email', e))
      throw new InternalServerErrorException('Error while sending email')
    }

    return {
      message: `Invite for ${email} successfully sent`
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
    const runner = this.neogmaService.createRunner()

    const result = await runner.run(
      this.workspaceQueryService.getWorkspaceAccessListQuery(),
      {
        workspaceId,
        role: USER_ROLE_EDITOR
      },
      transaction
    )

    const accessMap: Record<string, string[]> = {}

    for (const record of result.records) {
      const projectId = record.get('projectId')
      accessMap[projectId] = record.get('userIds') || []
    }

    return accessMap
  }

  async getInvitedUserList(workspaceId: string, transaction: Transaction): Promise<IUserClaims[]> {
    const runner = this.neogmaService.createRunner()

    const result = await runner.run(
      this.workspaceQueryService.getWorkspaceUserListQuery(),
      {
        workspaceId,
        role: USER_ROLE_EDITOR
      },
      transaction
    )

    return result.records.map((record) => ({
      id: record.get('id'),
      login: record.get('login')
    })) as IUserClaims[]
  }

  async revokeAccessList(workspaceId: string, userIds: string[], transaction: Transaction) {
    const runner = this.neogmaService.createRunner()

    for (const userId of userIds) {
      isDevMode(() => Logger.log(`[Revoke access LOG]: Check other workspaces for user ${userId}`))
      const countsResult = await runner.run(
        this.workspaceQueryService.getUserRoleCountsOutsideWorkspaceQuery(),
        { userId, workspaceId },
        transaction
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
        // @TODO: must be tested
        await runner.run(
          this.workspaceQueryService.getRemoveWorkspaceRelationQuery(),
          { userId, workspaceId },
          transaction
        )
        // @TODO: must be tested
        isDevMode(() => Logger.log(`[Revoke access LOG]: Remove project relation for user ${userId}`))
        await runner.run(
          this.workspaceQueryService.getRemoveProjectRelationsQuery(),
          { userId, workspaceId },
          transaction
        )
      } else {
        isDevMode(() =>
          Logger.log(`[Revoke access LOG]: No other entities found for user ${userId}, delete user data`)
        )
        await this.userService.delete({ userId, transaction })
      }
    }

    return { message: 'Access revoked where appropriate' }
  }
}
