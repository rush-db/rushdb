import { Injectable } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
// @TODO: Fix it
// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-ignore
import * as ms from 'ms'
import { Transaction } from 'neo4j-driver'
import { uuidv7 } from 'uuidv7'

import { getCurrentISO } from '@/common/utils/getCurrentISO'
import { toBoolean } from '@/common/utils/toBolean'
import { ProjectRepository } from '@/dashboard/project/model/project.repository'
import { ProjectService } from '@/dashboard/project/project.service'
import { CreateTokenDto } from '@/dashboard/token/dto/create-token.dto'
import { TokenEntity } from '@/dashboard/token/entity/token.entity'
import { TokenRepository } from '@/dashboard/token/model/token.repository'
import { ACCESS_WEIGHT, READ_ACCESS, WRITE_ACCESS, canWrite } from '@/dashboard/token/token.constants'
import { IUserClaims } from '@/dashboard/user/interfaces/user-claims.interface'
import { UserRepository } from '@/dashboard/user/model/user.repository'
import { WorkspaceRepository } from '@/dashboard/workspace/model/workspace.repository'

import * as crypto from 'node:crypto'

import type { TokenRow } from '@/database/sql/schema/types'
import { IProjectProperties } from '../project/model/project.interface'
import { IWorkspaceProperties } from '../workspace/model/workspace.interface'

@Injectable()
export class TokenService {
  constructor(
    private readonly tokenRepository: TokenRepository,
    private readonly configService: ConfigService,
    private readonly projectService: ProjectService,
    private readonly projectRepository: ProjectRepository,
    private readonly workspaceRepository: WorkspaceRepository,
    private readonly userRepository: UserRepository
  ) {}

  normalize(row: TokenRow): TokenEntity {
    return new TokenEntity(row.id, row.name, row.created, row.expiration, row.value, row.description)
  }

  encryptTokenData(tokenData: string): string {
    const encryptionKey = this.configService.get('RUSHDB_AES_256_ENCRYPTION_KEY')
    const iv = crypto.randomBytes(16)
    const cipher = crypto.createCipheriv('aes-256-cbc', encryptionKey, iv)
    return iv.toString('hex') + cipher.update(tokenData, 'utf8', 'base64') + cipher.final('base64')
  }

  decrypt(encrypted: string): string {
    const encryptionKey = this.configService.get('RUSHDB_AES_256_ENCRYPTION_KEY')
    const iv = encrypted.substring(0, 32)
    const cipherText = encrypted.substring(32)
    const decipher = crypto.createDecipheriv('aes-256-cbc', encryptionKey, Buffer.from(iv, 'hex'))
    const decrypted = decipher.update(cipherText, 'base64', 'utf8')
    return decrypted + decipher.final('utf8')
  }

  async createToken(
    properties: CreateTokenDto,
    projectId: string,
    _transaction?: Transaction
  ): Promise<TokenEntity> {
    const currentTime = getCurrentISO()
    const id = uuidv7()
    const { name, description = '', expiration: expirationRaw = '30d' } = properties
    const expiration = expirationRaw === '*' ? -1 : ms(expirationRaw as string)

    await this.projectService.getProject(projectId)

    const token = this.encryptTokenData(id)
    const level = (properties as any).level ?? WRITE_ACCESS

    const tokenRow = await this.tokenRepository.create({
      id,
      name,
      description,
      expiration,
      created: currentTime,
      value: token,
      projectId,
      level,
      ...(properties.consentId ? { consentId: properties.consentId } : {})
    })

    return this.normalize(tokenRow)
  }

  async deleteToken(tokenId: string, _transaction?: Transaction): Promise<boolean | undefined> {
    const deleted = await this.tokenRepository.delete(tokenId)
    return deleted ? true : undefined
  }

  isTokenExpired(tokenInstance: Pick<TokenRow, 'expiration' | 'created'>): boolean {
    const { expiration, created } = tokenInstance
    const currentTime = new Date()
    const creationTime = new Date(created)
    const validTill = creationTime.getTime() + expiration
    return expiration === -1 ? false : validTill < currentTime.getTime()
  }

  async validateToken({
    tokenId,
    transaction: _transaction
  }: {
    tokenId: string
    transaction?: Transaction
  }): Promise<{
    hasAccess: boolean
    accessLevel?: typeof READ_ACCESS | typeof WRITE_ACCESS
    canWrite?: boolean
    projectId: string
    project: IProjectProperties
    workspaceId: string
    workspace: IWorkspaceProperties
  }> {
    const row = await this.tokenRepository.findTokenWithProjectAndWorkspace(tokenId)

    if (!row) {
      return {
        hasAccess: false,
        projectId: undefined,
        project: undefined,
        workspaceId: undefined,
        workspace: undefined
      }
    }

    const { token, project, workspace } = row
    const level = token.level as typeof READ_ACCESS | typeof WRITE_ACCESS

    const isExpired = this.isTokenExpired(token)

    if (!level || isExpired) {
      return {
        hasAccess: false,
        projectId: undefined,
        project: undefined,
        workspaceId: undefined,
        workspace: undefined
      }
    }

    const hasRoleWeight = toBoolean(ACCESS_WEIGHT[level])

    return {
      hasAccess: !isExpired && hasRoleWeight,
      accessLevel: level,
      canWrite: canWrite(level),
      projectId: project.id,
      project: project as unknown as IProjectProperties,
      workspaceId: workspace.id,
      workspace: workspace as unknown as IWorkspaceProperties
    }
  }

  async verifyIntegrity({
    user,
    projectId,
    workspaceId,
    transaction: _transaction
  }: {
    user: IUserClaims
    workspaceId?: string
    projectId?: string
    transaction?: Transaction
  }): Promise<{
    hasAccess: boolean
    projectId?: string
    project?: IProjectProperties
    workspaceId?: string
    workspace?: IWorkspaceProperties
  }> {
    let projectRow = projectId ? await this.projectRepository.findById(projectId) : undefined

    const resolvedWorkspaceId = workspaceId ?? projectRow?.workspaceId
    const workspaceRow =
      resolvedWorkspaceId ? await this.workspaceRepository.findById(resolvedWorkspaceId) : undefined

    if (!workspaceRow) {
      return { hasAccess: false }
    }

    const memberRole = await this.userRepository.getUserRoleInWorkspace(user.id, workspaceRow.id)
    const hasAccess = !!memberRole

    return {
      hasAccess,
      projectId: projectRow?.id,
      project: projectRow as unknown as IProjectProperties,
      workspaceId: workspaceRow?.id,
      workspace: workspaceRow as unknown as IWorkspaceProperties
    }
  }

  async getTokenNode(id: string, _transaction?: Transaction): Promise<TokenRow | undefined> {
    return this.tokenRepository.findById(id)
  }

  async getTokensList(projectId: string, _transaction?: Transaction) {
    const rows = await this.tokenRepository.findByProjectId(projectId)
    return rows.map((row) => ({
      ...this.normalize(row).toJson(),
      expired: this.isTokenExpired(row)
    }))
  }

  async findLiveTokenByConsentAndProject(
    consentId: string,
    projectId: string
  ): Promise<TokenRow | undefined> {
    return this.tokenRepository.findLiveTokenByConsentAndProject(consentId, projectId)
  }

  async deleteByConsentId(consentId: string): Promise<void> {
    return this.tokenRepository.deleteByConsentId(consentId)
  }
}
