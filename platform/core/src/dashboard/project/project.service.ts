import { BadRequestException, forwardRef, Inject, Injectable } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { Transaction } from 'neo4j-driver'
import { uuidv7 } from 'uuidv7'

import { getCurrentISO } from '@/common/utils/getCurrentISO'
import { EntityService } from '@/core/entity/entity.service'
import { EntityRepository } from '@/core/entity/model/entity.repository'
import { PropertyService } from '@/core/property/property.service'
import { removeUndefinedKeys } from '@/core/property/property.utils'
import { CreateProjectDto } from '@/dashboard/project/dto/create-project.dto'
import { ProjectEntity } from '@/dashboard/project/entity/project.entity'
import { IProjectProperties, TProjectInstance } from '@/dashboard/project/model/project.interface'
import { ProjectRepository } from '@/dashboard/project/model/project.repository'
import { ProjectQueryService } from '@/dashboard/project/project-query.service'
import { TProjectCustomDbPayload, TProjectStats } from '@/dashboard/project/project.types'
import { USER_ROLE_OWNER } from '@/dashboard/user/interfaces/user.constants'
import { toNative } from '@/database/neogma/neogma-data.interceptor'
import { NeogmaService } from '@/database/neogma/neogma.service'
import * as crypto from 'node:crypto'

@Injectable()
export class ProjectService {
  constructor(
    private readonly configService: ConfigService,
    private readonly neogmaService: NeogmaService,
    private readonly projectRepository: ProjectRepository,
    private readonly entityRepository: EntityRepository,
    private readonly projectQueryService: ProjectQueryService,
    @Inject(forwardRef(() => EntityService))
    private readonly entityService: EntityService,
    @Inject(forwardRef(() => PropertyService))
    private readonly propertyService: PropertyService
  ) {}

  normalize(node: TProjectInstance) {
    return new ProjectEntity(node.id, node.name, node.created, node.description, node.edited, node.customDb)
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

    // add check for plan here and selfhosted prop.
    const customDb = this.attachCustomDb(properties.customDb)

    const projectNode = await this.projectRepository.model.createOne(
      {
        id,
        name,
        description,
        ...(customDb && { customDb }),
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

  attachCustomDb(payload?: TProjectCustomDbPayload): string | null {
    // add test connection here like ping-pong to the db
    if (!payload || !payload.url || !payload.username || !payload.password) {
      return null
    }

    return this.encryptCustomDb(payload)
  }

  encryptCustomDb(payload: TProjectCustomDbPayload): string {
    const encryptionKey = this.configService.get('RUSHDB_AES_256_ENCRYPTION_KEY')
    const iv = crypto.randomBytes(16)
    const customDbString = JSON.stringify(payload)

    const cipher = crypto.createCipheriv('aes-256-cbc', encryptionKey, iv)

    return iv.toString('hex') + cipher.update(customDbString, 'utf8', 'base64') + cipher.final('base64')
  }

  decryptCustomDb(encrypted: string): TProjectCustomDbPayload {
    const encryptionKey = this.configService.get('RUSHDB_AES_256_ENCRYPTION_KEY')
    const iv = encrypted.substring(0, 32)
    const cipherText = encrypted.substring(32)

    const decipher = crypto.createDecipheriv('aes-256-cbc', encryptionKey, Buffer.from(iv, 'hex'))
    const decrypted = decipher.update(cipherText, 'base64', 'utf8')
    const resultString = decrypted + decipher.final('utf8')

    return JSON.parse(resultString) as TProjectCustomDbPayload
  }

  async deleteProject(id: string, transaction: Transaction): Promise<boolean> {
    const projectNode = await this.projectRepository.model.findOne({
      where: { id },
      throwIfNotFound: false,
      session: transaction
    })

    if (projectNode) {
      projectNode['deleted'] = getCurrentISO()
      await projectNode.save()
    }

    // @FYI: UWAGA - keep it without await.
    this.cleanUpProject(id).then(() => projectNode.delete())

    return true
  }

  async cleanUpProject(id: string) {
    const session = this.neogmaService.createSession()
    const transaction = session.beginTransaction()
    const queryRunner = this.neogmaService.createRunner()

    try {
      await queryRunner
        .run(
          this.projectQueryService.removeProjectQuery(),
          {
            projectId: id
          },
          transaction
        )
        .then(
          async () =>
            await this.propertyService.deleteOrphanProps({
              projectId: id,
              queryRunner,
              transaction
            })
        )
    } catch (e) {
      console.log('[cleanUpProject ERROR]', e)
      if (transaction.isOpen()) {
        console.log('[ROLLBACK TRANSACTION]: Project service')
        await transaction.rollback()
      }
    } finally {
      await transaction.commit()
      await transaction.close().then(() => session.close())
    }
  }

  async recomputeProjectNodes(projectId: string, transaction: Transaction): Promise<string> {
    const projectNode = await this.getProjectNode(projectId, transaction)
    const projectNodePayload = JSON.stringify(await this.getNodesCount(projectId, transaction))
    projectNode.stats = projectNodePayload

    await projectNode.save()
    return projectNodePayload
  }

  async updateProject(
    id: string,
    projectProperties: Partial<IProjectProperties>,
    transaction: Transaction
  ): Promise<ProjectEntity> {
    const projectNode = await this.getProjectNode(id, transaction)

    if (!projectNode) {
      throw new BadRequestException(`Project ${id} not found`)
    }

    const { created, edited, ...restProperties } = projectProperties
    const fieldsToUpdate = removeUndefinedKeys(restProperties)

    const updateField = async (key: string, value: IProjectProperties[keyof IProjectProperties]) => {
      if (projectNode[key] !== value) {
        projectNode[key] = value
      }
    }

    await Promise.all(
      Object.entries<IProjectProperties[keyof IProjectProperties]>(fieldsToUpdate).map(
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
    transaction
  }: {
    projectId: string
    userId: string
    transaction: Transaction
  }) {
    await transaction.run(this.projectQueryService.grantUserAccessQuery(), {
      projectId,
      userId
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
    const queryRunner = this.neogmaService.createRunner()

    const result = await queryRunner.run(
      this.projectQueryService.projectRelatedUserIdsQuery(),
      {
        projectId
      },
      transaction
    )

    const actualAccessList = result.records.map((record) => record.get('usersId')).flat() as string[]
    const usersToAddAccess: Set<string> = new Set()
    const usersToRevokeAccess: Set<string> = new Set()

    userIdsToVerify.forEach((incomeUserId) =>
      actualAccessList.forEach((actualUserId) => {
        if (!actualAccessList.includes(incomeUserId)) {
          usersToAddAccess.add(incomeUserId)
        }

        if (!userIdsToVerify.includes(actualUserId)) {
          usersToRevokeAccess.add(actualUserId)
        }
      })
    )

    const grantAccessList = [...usersToAddAccess]
    const revokeAccessList = [...usersToRevokeAccess]

    await Promise.all([
      grantAccessList.map(
        async (userId) =>
          await this.grantUserAccessToProject({
            projectId,
            userId,
            transaction
          })
      ),
      revokeAccessList.map(
        async (userId) =>
          await this.revokeUserAccessToProject({
            projectId,
            userId,
            transaction
          })
      )
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

  async getProjectsByWorkspaceId(id: string, transaction: Transaction): Promise<ProjectEntity[]> {
    const queryRunner = this.neogmaService.createRunner()
    return await queryRunner
      .run(this.projectQueryService.getProjectsByWorkspaceId(), { id }, transaction)
      .then(({ records }) => records[0].get('projects'))
  }

  async getProjectsProperties(id: string, transaction: Transaction): Promise<IProjectProperties[]> {
    const queryRunner = this.neogmaService.createRunner()
    return await queryRunner
      .run(this.projectQueryService.getProjectsByWorkspaceId(), { id }, transaction)
      .then(({ records }) =>
        records[0].get('projects').map((project) => project.properties as IProjectProperties)
      )
  }

  async getNodesCount(projectId: string, transaction: Transaction): Promise<TProjectStats> {
    const queryRunner = this.neogmaService.createRunner()

    return await queryRunner
      .run(
        this.projectQueryService.getProjectStatsById(),
        {
          id: projectId
        },
        transaction
      )
      .then((result) => result.records[0])
      .then((record) => ({
        records: toNative(record.get('entities')),
        properties: toNative(record.get('properties'))
      }))
  }
}
