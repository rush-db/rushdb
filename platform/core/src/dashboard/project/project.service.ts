import { BadRequestException, forwardRef, Inject, Injectable, Logger } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { Transaction } from 'neo4j-driver'
import { uuidv7 } from 'uuidv7'

import { getCurrentISO } from '@/common/utils/getCurrentISO'
import { isDevMode } from '@/common/utils/isDevMode'
import { EmbeddingIndexRepository } from '@/core/ai/embedding-index.repository'
import { PropertyService } from '@/core/property/property.service'
import { removeUndefinedKeys } from '@/core/property/property.utils'
import { MailService } from '@/dashboard/mail/mail.service'
import { CreateProjectDto } from '@/dashboard/project/dto/create-project.dto'
import { ProjectEntity } from '@/dashboard/project/entity/project.entity'
import { IRawProjectProperties } from '@/dashboard/project/model/project.interface'
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

import type { ProjectRow } from '@/database/sql/schema/types'

@Injectable()
export class ProjectService {
  constructor(
    private readonly configService: ConfigService,
    private readonly neogmaService: NeogmaService,
    private readonly neogmaDynamicService: NeogmaDynamicService,
    private readonly embeddingIndexRepository: EmbeddingIndexRepository,
    private readonly projectRepository: ProjectRepository,
    private readonly projectQueryService: ProjectQueryService,
    @Inject(forwardRef(() => PropertyService))
    private readonly propertyService: PropertyService,
    @Inject(forwardRef(() => MailService))
    private readonly mailService: MailService
  ) {}

  normalize(row: ProjectRow): ProjectEntity {
    return new ProjectEntity(row)
  }

  async createProject(
    properties: CreateProjectDto,
    workspaceId: string,
    userId: string,
    _transaction?: Transaction
  ): Promise<ProjectEntity> {
    const currentTime = getCurrentISO()
    const id = uuidv7()
    const { name, description = '' } = properties

    const customDb = await this.attachCustomDb(properties.customDb)

    const projectRow = await this.projectRepository.create({
      id,
      name,
      description,
      workspaceId,
      ...(customDb && { customDb }),
      created: currentTime
    })

    await this.projectRepository.grantAccess({
      projectId: id,
      userId,
      role: USER_ROLE_OWNER,
      since: currentTime
    })

    return this.normalize(projectRow)
  }

  async dropProjectSubscription(id: string, _transaction?: Transaction) {
    const updated = await this.projectRepository.update(id, {
      edited: getCurrentISO()
    })
    return this.normalize(updated)
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
    return JSON.parse(decrypted + decipher.final('utf8')) as T
  }

  async deleteProject(
    id: string,
    transaction: Transaction,
    shouldStoreCustomDbData?: boolean
  ): Promise<boolean> {
    const projectRow = await this.projectRepository.findByIdIncludingDeleted(id)

    if (!projectRow) {
      return true
    }

    if (projectRow.customDb && !shouldStoreCustomDbData) {
      const customDbPayload = this.decryptSensitiveData<TProjectCustomDbPayload>(projectRow.customDb)
      this.cleanUpRemoteProject(id, customDbPayload).catch((e) => {
        isDevMode(() => Logger.error("[cleanUpRemoteProject ERROR]: Can't delete remote project data", e))
      })
    } else if (!projectRow.customDb) {
      this.cleanUpProject(id)
    }

    await this.embeddingIndexRepository.deleteByProjectId(id)
    await this.projectRepository.delete(id)
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
        .run(this.projectQueryService.removeRemoteDbDataQuery(), { projectId: id }, transaction)
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
      await transaction.close()
      await session.close()
    }
  }

  async cleanUpProject(projectId: string) {
    const session = this.neogmaService.createSession('cleanUpProject')
    const transaction = session.beginTransaction()

    try {
      await transaction.run(this.projectQueryService.removeProjectQuery(), { projectId })
      await this.propertyService.deleteOrphanProps({ projectId, transaction })
    } catch (e) {
      Logger.log('[cleanUpProject ERROR]', e)
      if (transaction.isOpen()) {
        Logger.log('[ROLLBACK TRANSACTION]: Project service')
        await transaction.rollback()
      }
    } finally {
      await transaction.commit()
      await transaction.close()
      await this.neogmaService.closeSession(session)
    }
  }

  async recomputeProjectNodes(
    projectId: string,
    transaction: Transaction,
    customTx: Transaction = transaction
  ): Promise<string> {
    const projectNodePayload = JSON.stringify(await this.getNodesCount(projectId, customTx))
    await this.projectRepository.update(projectId, { stats: projectNodePayload, edited: getCurrentISO() })
    return projectNodePayload
  }

  async updateProject(
    id: string,
    projectProperties: Partial<IRawProjectProperties>,
    _transaction?: Transaction
  ): Promise<ProjectEntity> {
    const projectRow = await this.projectRepository.findByIdIncludingDeleted(id)
    if (!projectRow) {
      throw new BadRequestException(`Project ${id} not found`)
    }

    const { created, edited, ...restProperties } = projectProperties
    const fieldsToUpdate = removeUndefinedKeys(restProperties) as any

    // Handle customDb update
    if (fieldsToUpdate.customDb && typeof fieldsToUpdate.customDb === 'object') {
      const customDb = await this.attachCustomDb(fieldsToUpdate.customDb)
      if (customDb) {
        fieldsToUpdate.customDb = customDb
      }
    } else if (fieldsToUpdate.customDb && typeof fieldsToUpdate.customDb === 'string') {
      delete fieldsToUpdate.customDb // don't overwrite hash with the same hash
    }

    fieldsToUpdate.edited = getCurrentISO()
    const updated = await this.projectRepository.update(id, fieldsToUpdate)
    return this.normalize(updated)
  }

  async grantUserAccessToProject({
    projectId,
    userId,
    role,
    transaction: _transaction
  }: {
    projectId: string
    userId: string
    role: TUserRoles
    transaction?: Transaction
  }) {
    const since = getCurrentISO()
    await this.projectRepository.grantAccess({ projectId, userId, role, since })
  }

  async revokeUserAccessToProject({
    projectId,
    userId,
    transaction: _transaction
  }: {
    projectId: string
    userId: string
    transaction?: Transaction
  }) {
    await this.projectRepository.revokeAccess(projectId, userId)
  }

  async processUserAccess({
    userIdsToVerify,
    projectId,
    transaction: _transaction
  }: {
    userIdsToVerify: string[]
    projectId: string
    transaction?: Transaction
  }): Promise<string[]> {
    const role = USER_ROLE_EDITOR

    const actualAccessRows = await this.projectRepository.getAccessListByProjectId(projectId, role)
    const actualAccessList = actualAccessRows.map((r) => r.userId)

    const usersToAddAccess: Set<string> = new Set()
    const usersToRevokeAccess: Set<string> = new Set()

    if (!userIdsToVerify.length) {
      // nothing to do
    } else {
      userIdsToVerify.forEach((incomeUserId) => {
        if (!actualAccessList.includes(incomeUserId)) {
          usersToAddAccess.add(incomeUserId)
        }
      })

      actualAccessList.forEach((actualUserId) => {
        if (!userIdsToVerify.includes(actualUserId)) {
          usersToRevokeAccess.add(actualUserId)
        }
      })
    }

    await Promise.all([
      ...[...usersToAddAccess].map(async (userId) => {
        isDevMode(() =>
          Logger.log(`[Add user access LOG]: Add user ${userId} access to the project ${projectId}`)
        )
        return this.grantUserAccessToProject({ projectId, userId, role })
      }),
      ...[...usersToRevokeAccess].map(async (userId) => {
        isDevMode(() =>
          Logger.log(`[Revoke user access LOG]: Revoke user ${userId} access to the project ${projectId}`)
        )
        return this.revokeUserAccessToProject({ projectId, userId })
      })
    ])

    return userIdsToVerify
  }

  async getProjectNode(id: string, _transaction?: Transaction): Promise<ProjectRow | undefined> {
    return this.projectRepository.findByIdIncludingDeleted(id)
  }

  async getProject(id: string, _transaction?: Transaction): Promise<ProjectEntity> {
    const row = await this.projectRepository.findById(id)
    return row ? this.normalize(row) : undefined
  }

  /** Alias for getProject */
  async getProjectById(id: string, _transaction?: Transaction): Promise<ProjectEntity | undefined> {
    return this.getProject(id, _transaction)
  }

  async getProjectsByWorkspaceId(
    workspaceId: string,
    userId: string,
    _transaction?: Transaction
  ): Promise<ProjectEntity[]> {
    const rows = await this.projectRepository.findForUser(workspaceId, userId)
    return rows.map((row) => this.normalize(row))
  }

  async getProjectsProperties(workspaceId: string, _transaction?: Transaction) {
    return this.projectRepository.findByWorkspaceId(workspaceId)
  }

  async getNodesCount(projectId: string, transaction: Transaction): Promise<TProjectStats> {
    return await transaction
      .run(this.projectQueryService.getProjectStatsById(), { id: projectId })
      .then((result) => result.records[0])
      .then((record) => ({
        records: toNative(record.get('entities')),
        properties: toNative(record.get('properties'))
      }))
  }

  async linkUserToProject(userId: string, projectId: string, since: string, _transaction?: Transaction) {
    await this.projectRepository.grantAccess({
      projectId,
      userId,
      role: USER_ROLE_EDITOR,
      since
    })
  }
}
