import { Injectable } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
// @TODO: Fix it
// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-ignore
import * as ms from 'ms'
import { Transaction } from 'neo4j-driver'
import { uuidv7 } from 'uuidv7'

import { getCurrentISO } from '@/common/utils/getCurrentISO'
import { CreateTokenDto } from '@/dashboard/token/dto/create-token.dto'
import { TokenEntity } from '@/dashboard/token/entity/token.entity'
import { TTokenInstance } from '@/dashboard/token/model/token.interface'
import { TokenRepository } from '@/dashboard/token/model/token.repository'
import { TokenQueryService } from '@/dashboard/token/token-query.service'
import { ACCESS_WEIGHT, READ_ACCESS, WRITE_ACCESS } from '@/dashboard/token/token.constants'
import { NeogmaService } from '@/database/neogma/neogma.service'

import * as crypto from 'node:crypto'
import { ProjectService } from '@/dashboard/project/project.service'
import { toBoolean } from '@/common/utils/toBolean'
import { MixedTypeResult, ServerSettings } from '@/common/types/prefix'
import {
  attachMixedProperties,
  extractMixedPropertiesFromToken,
  getNormalizedPrefix,
  getPrefixedPlan
} from '@/common/utils/tokenUtils'
import { WorkspaceService } from '@/dashboard/workspace/workspace.service'

@Injectable()
export class TokenService {
  constructor(
    private readonly neogmaService: NeogmaService,
    private readonly tokenRepository: TokenRepository,
    private readonly tokenQueryService: TokenQueryService,
    private readonly configService: ConfigService,
    private readonly projectService: ProjectService,
    private readonly workspaceService: WorkspaceService
  ) {}

  normalize(node: TTokenInstance) {
    return new TokenEntity(
      node.id,
      node.name,
      node.created,
      node.expiration,
      node.value,
      node.description,
      node.prefixValue
    )
  }

  encryptTokenData(tokenData) {
    const encryptionKey = this.configService.get('RUSHDB_AES_256_ENCRYPTION_KEY')
    const iv = crypto.randomBytes(16)

    const cipher = crypto.createCipheriv('aes-256-cbc', encryptionKey, iv)

    return iv.toString('hex') + cipher.update(tokenData, 'utf8', 'base64') + cipher.final('base64')
  }

  decrypt(encrypted) {
    const [_, rawToken] = extractMixedPropertiesFromToken(encrypted)
    const encryptionKey = this.configService.get('RUSHDB_AES_256_ENCRYPTION_KEY')
    const iv = rawToken.substring(0, 32)
    const cipherText = rawToken.substring(32)

    const decipher = crypto.createDecipheriv('aes-256-cbc', encryptionKey, Buffer.from(iv, 'hex'))
    const decrypted = decipher.update(cipherText, 'base64', 'utf8')
    return decrypted + decipher.final('utf8')
  }

  async createToken(
    properties: CreateTokenDto,
    projectId: string,
    transaction: Transaction
  ): Promise<TokenEntity> {
    const currentTime = getCurrentISO()
    const id = uuidv7()
    const { name, description = '', expiration: expirationRaw = '30d' } = properties
    const expiration = expirationRaw === '*' ? -1 : ms(expirationRaw as string)
    const projectNode = await this.projectService.getProject(projectId, transaction)
    const workspaceNode = await this.workspaceService.getWorkspaceByProject(projectId, transaction)
    // @TODO: managedDb should be extracted from this point
    const { customDb } = projectNode.toJson()
    const { planId, isSubscriptionCancelled } = workspaceNode
    const selfHosted = toBoolean(this.configService.get('RUSHDB_SELF_HOSTED'))
    const tokenPrefix = {
      customDb: Boolean(customDb),
      selfHosted,
      canceled: isSubscriptionCancelled
    } as ServerSettings
    const prefixString = attachMixedProperties(getPrefixedPlan(planId as string), tokenPrefix)

    const token = this.encryptTokenData(id)
    const tokenNode = await this.tokenRepository.model.createOne(
      {
        id,
        name,
        description,
        expiration,
        created: currentTime,
        value: token,
        prefixValue: prefixString
      },
      { session: transaction }
    )

    await tokenNode.relateTo({
      alias: 'Projects',
      where: { id: projectId },
      session: transaction,
      properties: { Level: WRITE_ACCESS }
    })

    return this.normalize(tokenNode)
  }

  async deleteToken(tokenId: string, transaction: Transaction): Promise<boolean | undefined> {
    const removeResult = await this.tokenRepository.model.delete({
      where: { id: tokenId },
      detach: true,
      session: transaction
    })

    if (removeResult === 0) {
      return undefined
    }

    return true
  }

  isTokenExpired(tokenInstance: TTokenInstance) {
    const { expiration, created } = tokenInstance
    const currentTime = new Date()
    const creationTime = new Date(created)
    const validTill = creationTime.getTime() + expiration

    return expiration === -1 ? false : validTill < currentTime.getTime()
  }

  isTokenPrefixMalformed(tokenInstance: TTokenInstance, incomingTokenPrefix: MixedTypeResult) {
    const [settings] = incomingTokenPrefix

    if (settings === null && !tokenInstance.prefixValue) {
      return false
    } else if (settings && !tokenInstance.prefixValue) {
      return true
    } else if (settings === null && tokenInstance.prefixValue) {
      return true
    }

    const storedSettings = getNormalizedPrefix(tokenInstance.prefixValue)

    if (storedSettings === null) {
      return true
    }

    return Object.keys(storedSettings).every((key) => storedSettings[key] === settings[key])
  }

  async validateToken({
    tokenId,
    transaction,
    prefixData
  }: {
    tokenId: string
    transaction: Transaction
    prefixData: MixedTypeResult
  }): Promise<{
    hasAccess: boolean
    projectId: string
    workspaceId: string
  }> {
    const queryRunner = this.neogmaService.createRunner()

    const { token, project, workspace, level } = await queryRunner
      .run(
        this.tokenQueryService.traverseTokenData(),
        {
          tokenId
        },
        transaction
      )
      .then((result) => {
        return {
          // @FYI: If returning plain nodes from query we need to use '.properties' to access their properties
          token: result.records[0]?.get('token').properties,
          project: result.records[0]?.get('project').properties,
          workspace: result.records[0]?.get('workspace').properties,
          level: result.records[0]?.get('level')
        }
      })

    const isMalformedPrefix = this.isTokenPrefixMalformed(token, prefixData)

    if (isMalformedPrefix) {
      return {
        hasAccess: false,
        projectId: undefined,
        workspaceId: undefined
      }
    }

    const currentTokenRole = level as typeof READ_ACCESS | typeof WRITE_ACCESS

    if (!currentTokenRole) {
      return {
        hasAccess: false,
        projectId: undefined,
        workspaceId: undefined
      }
    }

    const isExpired = this.isTokenExpired(token)

    if (isExpired) {
      return {
        hasAccess: false,
        projectId: undefined,
        workspaceId: undefined
      }
    }

    const hasRoleWeight = Boolean(ACCESS_WEIGHT[currentTokenRole])
    // const minimalAccessLevel = ACCESS_WEIGHT[accessLevel];

    return {
      hasAccess: !isExpired && hasRoleWeight,
      projectId: project.id,
      workspaceId: workspace.id
    }
  }

  async getTokenNode(id: string, transaction: Transaction) {
    const queryRunner = this.neogmaService.createRunner()

    const token = await this.neogmaService
      .createBuilder()
      .match({
        model: this.tokenRepository.model,
        where: { id },
        identifier: 'i'
      })
      .return('i')
      .run(queryRunner, transaction)

    const tokenNode = token.records[0]?.get('i')

    if (!tokenNode) {
      return
    }

    return this.tokenRepository.model.buildFromRecord(tokenNode)
  }

  async getTokensList(projectId: string, transaction: Transaction) {
    const related = await this.tokenRepository.model.findRelationships({
      alias: 'Projects',
      where: {
        target: {
          id: projectId
        }
      },
      session: transaction
    })

    return related.map(({ source }) => ({
      ...this.normalize(source).toJson(),
      expired: this.isTokenExpired(source)
    }))
  }
}
