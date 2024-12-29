import { BadRequestException, forwardRef, Inject, Injectable } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { Transaction } from 'neo4j-driver'
import { uuidv7 } from 'uuidv7'

import { getCurrentISO } from '@/common/utils/getCurrentISO'
import { toBoolean } from '@/common/utils/toBolean'
import { removeUndefinedKeys } from '@/core/property/property.utils'
import { ProjectService } from '@/dashboard/project/project.service'
import { TProjectStats } from '@/dashboard/project/project.types'
import { USER_ROLE_OWNER } from '@/dashboard/user/interfaces/user.constants'
import { UserRepository } from '@/dashboard/user/model/user.repository'
import { UserService } from '@/dashboard/user/user.service'
import { CreateWorkspaceDto } from '@/dashboard/workspace/dto/create-workspace.dto'
import { Workspace } from '@/dashboard/workspace/entity/workspace.entity'
import {
  TWorkspaceInstance,
  TWorkspaceLimits,
  TWorkspaceProperties
} from '@/dashboard/workspace/model/workspace.interface'
import { WorkspaceRepository } from '@/dashboard/workspace/model/workspace.repository'
import {
  WORKSPACE_LIMITS_DEFAULT,
  WORKSPACE_LIMITS_PRO,
  WORKSPACE_LIMITS_WHITE_LABEL
} from '@/dashboard/workspace/workspace.constants'
import { NeogmaService } from '@/database/neogma/neogma.service'

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
    @Inject(forwardRef(() => ProjectService))
    private readonly projectService: ProjectService,
    @Inject(forwardRef(() => UserRepository))
    private readonly userRepository: UserRepository,
    @Inject(forwardRef(() => UserService))
    private readonly userService: UserService
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

  async findUserWorkspace(login: string, transaction: Transaction): Promise<string> {
    const userNode = await this.userService.find(login, transaction)
    const userId = userNode.getId()

    const workspaceList = await this.getWorkspacesList(userId, transaction)

    // @TODO: Improve for multiple workspaces support
    return workspaceList[0].id
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

    return toBoolean(key) ? WORKSPACE_LIMITS_PRO : WORKSPACE_LIMITS_DEFAULT
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
        project.target.id
          ? await this.projectService.deleteProject(project.target.id, transaction)
          : undefined
      )
      .filter(Boolean)

    await workspace.delete({ detach: true })
    await Promise.all(projectsToDelete)

    return {
      message: `Workspace ${id} successfully deleted`
    }
  }
}
