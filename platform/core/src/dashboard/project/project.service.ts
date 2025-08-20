import { BadRequestException, forwardRef, Inject, Injectable, Logger } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { Transaction } from 'neo4j-driver'
import { uuidv7 } from 'uuidv7'

import { getCurrentISO } from '@/common/utils/getCurrentISO'
import { isDevMode } from '@/common/utils/isDevMode'
import { toBoolean } from '@/common/utils/toBolean'
import { PropertyService } from '@/core/property/property.service'
import { removeUndefinedKeys } from '@/core/property/property.utils'
import { MailService } from '@/dashboard/mail/mail.service'
import { CreateProjectDto } from '@/dashboard/project/dto/create-project.dto'
import { ProjectEntity } from '@/dashboard/project/entity/project.entity'
import {
  IProjectProperties,
  IRawProjectProperties,
  TProjectInstance
} from '@/dashboard/project/model/project.interface'
import { ProjectRepository } from '@/dashboard/project/model/project.repository'
import { ProjectQueryService } from '@/dashboard/project/project-query.service'
import { TProjectCustomDbPayload, TProjectStats } from '@/dashboard/project/project.types'
import { USER_ROLE_EDITOR, USER_ROLE_OWNER } from '@/dashboard/user/interfaces/user.constants'
import { TUserRoles } from '@/dashboard/user/model/user.interface'
import { toNative } from '@/database/interceptors/data.interceptor'
import { INeogmaConfig } from '@/database/neogma/neogma-config.interface'
import { NeogmaService } from '@/database/neogma/neogma.service'
import { NeogmaDynamicService } from '@/database/neogma-dynamic/neogma-dynamic.service'

import * as crypto from 'node:crypto'

@Injectable()
export class ProjectService {
  constructor(
    private readonly configService: ConfigService,
    private readonly neogmaService: NeogmaService,
    private readonly neogmaDynamicService: NeogmaDynamicService,
    private readonly projectRepository: ProjectRepository,
    private readonly projectQueryService: ProjectQueryService,
    @Inject(forwardRef(() => PropertyService))
    private readonly propertyService: PropertyService,
    @Inject(forwardRef(() => MailService))
    private readonly mailService: MailService
  ) {}

  normalize(node: TProjectInstance) {
    return new ProjectEntity(node)
  }

  async createProject(
    properties: CreateProjectDto,
    workspaceId: string,
    userId: string,
    transaction: Transaction
  ): Promise<ProjectEntity> {
    const currentTime = getCurrentISO()
    const id = uuidv7()
    const { name, description = '' } = properties

    const customDb = await this.attachCustomDb(properties.customDb)
    const managedDb =
      toBoolean(properties.managedDbConfig) &&
      toBoolean(properties.managedDbConfig.password) &&
      toBoolean(properties.managedDbConfig.region) &&
      toBoolean(properties.managedDbConfig.tier)

    const projectNode = await this.projectRepository.model.createOne(
      {
        id,
        name,
        description,
        ...(customDb && { customDb }),
        ...(managedDb && {
          managedDbPassword: this.encryptSensitiveData(properties.managedDbConfig.password),
          managedDbRegion: properties.managedDbConfig.region,
          managedDbTier: properties.managedDbConfig.tier,
          status: 'pending'
        }),
        created: currentTime
      },
      { session: transaction }
    )

    await projectNode.relateTo({
      alias: 'Workspaces',
      where: { id: workspaceId },
      session: transaction
    })

    await projectNode.relateTo({
      alias: 'Users',
      where: { id: userId },
      session: transaction,
      properties: { Since: projectNode.created, Role: USER_ROLE_OWNER }
    })

    await projectNode.save({ session: transaction })

    return this.normalize(projectNode)
  }

  async dropProjectSubscription(id: string, transaction: Transaction) {
    const projectNode = await this.getProjectNode(id, transaction)
    projectNode['isSubscriptionCancelled'] = true
    projectNode['edited'] = getCurrentISO()

    await projectNode.save()

    return this.normalize(projectNode)
  }

  async attachCustomDb(payload?: TProjectCustomDbPayload): Promise<string | null> {
    if (!payload || !payload.url || !payload.username || !payload.password) {
      return null
    }

    const config: INeogmaConfig = {
      url: payload.url,
      username: payload.username,
      password: payload.password
    }

    await this.neogmaDynamicService.validateConnection(config)

    return this.encryptSensitiveData<TProjectCustomDbPayload>(payload)
  }

  encryptSensitiveData<T>(payload: T): string {
    const encryptionKey = this.configService.get('RUSHDB_AES_256_ENCRYPTION_KEY')
    const iv = crypto.randomBytes(16)
    const customDbString = JSON.stringify(payload)

    const cipher = crypto.createCipheriv('aes-256-cbc', encryptionKey, iv)

    return iv.toString('hex') + cipher.update(customDbString, 'utf8', 'base64') + cipher.final('base64')
  }

  decryptSensitiveData<T>(encrypted: string): T {
    const encryptionKey = this.configService.get('RUSHDB_AES_256_ENCRYPTION_KEY')
    const iv = encrypted.substring(0, 32)
    const cipherText = encrypted.substring(32)

    const decipher = crypto.createDecipheriv('aes-256-cbc', encryptionKey, Buffer.from(iv, 'hex'))
    const decrypted = decipher.update(cipherText, 'base64', 'utf8')
    const resultString = decrypted + decipher.final('utf8')

    return JSON.parse(resultString) as T
  }

  async deleteProject(
    id: string,
    transaction: Transaction,
    shouldStoreCustomDbData?: boolean
  ): Promise<boolean> {
    const projectNode = await this.projectRepository.model.findOne({
      where: { id },
      throwIfNotFound: false,
      session: transaction
    })

    if (projectNode.customDb && !shouldStoreCustomDbData) {
      const customDbPayload = this.decryptSensitiveData<TProjectCustomDbPayload>(projectNode.customDb)

      await this.removeProjectOwnNode(id, transaction)
      this.cleanUpRemoteProject(id, customDbPayload).catch((e) => {
        isDevMode(() => Logger.error("[cleanUpRemoteProject ERROR]: Can't delete remote project data", e))
      })

      return true
    } else if (projectNode.customDb && shouldStoreCustomDbData) {
      await this.removeProjectOwnNode(id, transaction)

      return true
    }

    await this.removeProjectOwnNode(id, transaction)
    this.cleanUpProject(id)

    return true
  }

  async cleanUpRemoteProject(id: string, customDbPayload: TProjectCustomDbPayload) {
    isDevMode(() => Logger.log('[cleanUpRemoteProject LOG]: Create temp customDb runner'))
    const { runner, session, transaction } = await this.neogmaDynamicService.getTempRunner(
      id,
      customDbPayload
    )

    try {
      isDevMode(() => Logger.log('[cleanUpRemoteProject LOG]: Deleting all remote db data'))

      await runner
        .run(
          this.projectQueryService.removeRemoteDbDataQuery(),
          {
            projectId: id
          },
          transaction
        )
        .then(
          async () =>
            await this.propertyService.deleteOrphanProps({
              projectId: id,
              transaction
            })
        )
    } catch (e) {
      isDevMode(() =>
        Logger.error('[cleanUpRemoteProject ERROR]: failed to process cleanUpRemoteProject method', e)
      )

      if (transaction.isOpen()) {
        isDevMode(() => Logger.log('[ROLLBACK CUSTOM TRANSACTION]: cleanUpRemoteProject'))
        await transaction.rollback()
      }
    } finally {
      isDevMode(() => Logger.log('[COMMIT CUSTOM TRANSACTION]: cleanUpRemoteProject'))
      await transaction.commit()
      await transaction.close().then(() => session.close())
    }
  }

  async removeProjectOwnNode(projectId: string, transaction: Transaction) {
    try {
      isDevMode(() => Logger.log('[cleanUpProject LOG]: Running removeProjectOwnNode method'))
      await transaction.run(this.projectQueryService.removeProjectNodeQuery(), {
        projectId
      })
    } catch (e) {
      isDevMode(() =>
        Logger.error('[cleanUpProject ERROR]: failed to process removeProjectOwnNode method', e)
      )
    }
  }

  async cleanUpProject(projectId: string) {
    const session = this.neogmaService.createSession('cleanUpProject')
    const transaction = session.beginTransaction()

    try {
      await transaction.run(this.projectQueryService.removeProjectQuery(), {
        projectId
      })

      await this.propertyService.deleteOrphanProps({
        projectId,
        transaction
      })
    } catch (e) {
      Logger.log('[cleanUpProject ERROR]', e)
      if (transaction.isOpen()) {
        Logger.log('[ROLLBACK TRANSACTION]: Project service')
        await transaction.rollback()
      }
    } finally {
      await transaction.commit()
      await transaction.close().then(() => session.close())
    }
  }

  async recomputeProjectNodes(
    projectId: string,
    transaction: Transaction,
    customTx: Transaction = transaction
  ): Promise<string> {
    const projectNode = await this.getProjectNode(projectId, transaction)
    const projectNodePayload = JSON.stringify(await this.getNodesCount(projectId, customTx))
    projectNode.stats = projectNodePayload

    await projectNode.save()
    return projectNodePayload
  }

  async notifyRushDBAdmin(id: string, transaction: Transaction) {
    const projectNode = await this.getProjectNode(id, transaction)

    const managedDb =
      projectNode.managedDbPassword &&
      projectNode.managedDbPassword &&
      projectNode.managedDbTier &&
      projectNode.managedDbRegion

    if (!toBoolean(this.configService.get('RUSHDB_SELF_HOSTED')) && managedDb) {
      try {
        await this.mailService.notifyAdminAboutNewProject({
          projectId: id,
          name: projectNode.name,
          region: projectNode.managedDbRegion,
          tier: projectNode.managedDbTier,
          password: this.decryptSensitiveData(projectNode.managedDbPassword)
        })
      } catch (e) {
        isDevMode(() => Logger.error('[MAIL ERROR]: Failed to notify admin about managed project', e))
      }
    }
  }

  async updateProject(
    id: string,
    projectProperties: Partial<IRawProjectProperties>,
    transaction: Transaction
  ): Promise<ProjectEntity> {
    const projectNode = await this.getProjectNode(id, transaction)

    if (!projectNode) {
      throw new BadRequestException(`Project ${id} not found`)
    }

    const { created, edited, ...restProperties } = projectProperties
    const fieldsToUpdate = removeUndefinedKeys(restProperties)

    const updateField = async (key: string, value: IRawProjectProperties[keyof IRawProjectProperties]) => {
      if (key === 'customDb' && typeof value === 'string') {
        // do nothing for updating hash of customDb
        return
      } else if (key === 'customDb' && typeof value === 'object') {
        const customDb = await this.attachCustomDb(value)

        if (customDb) {
          projectNode[key] = customDb
        }
      } else if (projectNode[key] !== value) {
        projectNode[key] = value
      }
    }

    await Promise.all(
      Object.entries<IRawProjectProperties[keyof IRawProjectProperties]>(fieldsToUpdate).map(
        async ([key, value]) => await updateField(key, value)
      )
    )

    projectNode['edited'] = getCurrentISO()
    await projectNode.save()

    return this.normalize(projectNode)
  }

  async grantUserAccessToProject({
    projectId,
    userId,
    role,
    transaction
  }: {
    projectId: string
    userId: string
    role: TUserRoles
    transaction: Transaction
  }) {
    const since = getCurrentISO()

    await transaction.run(this.projectQueryService.grantUserAccessQuery(), {
      projectId,
      userId,
      role,
      since
    })
  }

  async revokeUserAccessToProject({
    projectId,
    userId,
    transaction
  }: {
    projectId: string
    userId: string
    transaction: Transaction
  }) {
    await transaction.run(this.projectQueryService.revokeUserAccessQuery(), {
      projectId,
      userId
    })
  }

  async processUserAccess({
    userIdsToVerify,
    projectId,
    transaction
  }: {
    userIdsToVerify: string[]
    projectId: string
    transaction: Transaction
  }): Promise<string[]> {
    const role = USER_ROLE_EDITOR

    const result = await transaction.run(this.projectQueryService.projectRelatedUserIdsQuery(), {
      projectId,
      role
    })

    const actualAccessList = result.records.map((record) => record.get('usersId')).flat() as string[]
    const usersToAddAccess: Set<string> = new Set()
    const usersToRevokeAccess: Set<string> = new Set()

    if (!userIdsToVerify.length) {
      actualAccessList.forEach((actualUserId) => {})
    } else {
      userIdsToVerify.forEach((incomeUserId) => {
        if (!actualAccessList.length) {
          usersToAddAccess.add(incomeUserId)
        }

        actualAccessList.forEach((actualUserId) => {
          if (!actualAccessList.includes(incomeUserId)) {
            usersToAddAccess.add(incomeUserId)
          }

          if (!userIdsToVerify.includes(actualUserId)) {
            usersToRevokeAccess.add(actualUserId)
          }
        })
      })
    }

    const grantAccessList = [...usersToAddAccess]
    const revokeAccessList = [...usersToRevokeAccess]

    await Promise.all([
      grantAccessList.map(async (userId) => {
        isDevMode(() =>
          Logger.log(`[Add user access LOG]: Add user ${userId} access to the project ${projectId}`)
        )

        return await this.grantUserAccessToProject({
          projectId,
          userId,
          role,
          transaction
        })
      }),
      revokeAccessList.map(async (userId) => {
        isDevMode(() =>
          Logger.log(`[Revoke user access LOG]: Revoke user ${userId} access to the project ${projectId}`)
        )

        return await this.revokeUserAccessToProject({
          projectId,
          userId,
          transaction
        })
      })
    ])

    return userIdsToVerify
  }

  async getProjectNode(id: string, transaction: Transaction) {
    const queryRunner = this.neogmaService.createRunner()

    const project = await this.neogmaService
      .createBuilder()
      .match({
        model: this.projectRepository.model,
        where: { id },
        identifier: 'i'
      })
      .return('i')
      .run(queryRunner, transaction)

    const projectNode = project.records[0]?.get('i')

    if (!projectNode) {
      return
    }

    return this.projectRepository.model.buildFromRecord(projectNode)
  }

  async getProject(id: string, transaction: Transaction) {
    const projectNode = await this.projectRepository.model.findOne({
      where: { id },
      session: transaction
    })

    return this.normalize(projectNode)
  }

  async getProjectsByWorkspaceId(
    id: string,
    userId: string,
    transaction: Transaction
  ): Promise<ProjectEntity[]> {
    return await transaction
      .run(this.projectQueryService.getUserRelatedProjects(), { id, userId })
      .then(({ records }) => records[0].get('projects'))
  }

  async getProjectsProperties(id: string, transaction: Transaction): Promise<IProjectProperties[]> {
    return await transaction
      .run(this.projectQueryService.getProjectsByWorkspaceId(), { id })
      .then(({ records }) =>
        records[0].get('projects').map((project) => project.properties as IProjectProperties)
      )
  }

  async getNodesCount(projectId: string, transaction: Transaction): Promise<TProjectStats> {
    return await transaction
      .run(this.projectQueryService.getProjectStatsById(), {
        id: projectId
      })
      .then((result) => result.records[0])
      .then((record) => ({
        records: toNative(record.get('entities')),
        properties: toNative(record.get('properties')),
        avgProperties: toNative(record.get('avg'))
      }))
  }

  async linkUserToProject(userId: string, projectId: string, since: string, transaction: Transaction) {
    await transaction.run(this.projectQueryService.getAttachUserToProjectQuery(), {
      userId,
      projectId,
      since,
      role: USER_ROLE_EDITOR
    })
  }
}
