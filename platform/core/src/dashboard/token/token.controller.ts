import {
  Body,
  Controller,
  Delete,
  Get,
  HttpCode,
  HttpStatus,
  NotFoundException,
  Param,
  Post,
  UseInterceptors,
  Request
} from '@nestjs/common'
import { ApiBearerAuth, ApiExcludeController, ApiParam, ApiTags } from '@nestjs/swagger'
import { Transaction } from 'neo4j-driver'

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { PlatformRequest } from '@/common/types/request'
import { TokenReadAccess } from '@/dashboard/auth/decorators/token-read-access.decorator'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { ChangeCorsInterceptor } from '@/dashboard/common/interceptors/change-cors.interceptor'
import { CreateTokenDto } from '@/dashboard/token/dto/create-token.dto'
import { ITokenProperties } from '@/dashboard/token/model/token.interface'
import { TokenService } from '@/dashboard/token/token.service'
import { USER_ROLE_OWNER } from '@/dashboard/user/interfaces/user.constants'
import { DataInterceptor } from '@/database/interceptors/data.interceptor'
import { TransactionDecorator } from '@/database/transaction.decorator'

@Controller('tokens')
@ApiExcludeController()
@UseInterceptors(
  TransformResponseInterceptor,
  NotFoundInterceptor,
  DataInterceptor,

  ChangeCorsInterceptor
)
export class TokenController {
  constructor(private readonly tokenService: TokenService) {}

  @Post()
  @ApiTags('Tokens')
  @ApiBearerAuth()
  @AuthGuard('project')
  @HttpCode(HttpStatus.CREATED)
  async createToken(
    @Body() createTokenProperties: CreateTokenDto,
    @TransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<ITokenProperties> {
    const projectId = request.projectId
    const token = await this.tokenService.createToken(createTokenProperties, projectId, transaction)

    return token.toJson()
  }

  @Get()
  @ApiTags('Tokens')
  @ApiBearerAuth()
  @AuthGuard('project')
  @HttpCode(HttpStatus.CREATED)
  async getTokensList(
    @TransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<Array<ITokenProperties & { expired: boolean }>> {
    const projectId = request.projectId

    return await this.tokenService.getTokensList(projectId, transaction)
  }

  // Introspection for the calling API key itself (SDKs/MCP use this to discover
  // the key's access level). API-key auth only — JWT sessions have no token.
  @Get('current')
  @ApiTags('Tokens')
  @ApiBearerAuth()
  @AuthGuard('project')
  @TokenReadAccess()
  @HttpCode(HttpStatus.OK)
  async getCurrentToken(
    @Request() request: PlatformRequest
  ): Promise<Omit<ITokenProperties, 'value'> & { expired: boolean }> {
    const authHeader = request.headers['authorization']
    const bearerToken = authHeader?.split(' ')[1]
    const isJwt = bearerToken?.split('.').length === 3

    if (!bearerToken || isJwt) {
      throw new NotFoundException('This endpoint is only available for API key authentication')
    }

    const tokenId = this.tokenService.decrypt(bearerToken)
    const row = await this.tokenService.getTokenNode(tokenId)

    if (!row || row.projectId !== request.projectId) {
      throw new NotFoundException('Token not found')
    }

    const { value: _value, ...properties } = this.tokenService.normalize(row).toJson()
    return { ...properties, expired: this.tokenService.isTokenExpired(row) }
  }

  @Get('all')
  @ApiTags('Tokens')
  @ApiBearerAuth()
  @AuthGuard('workspace', USER_ROLE_OWNER)
  @HttpCode(HttpStatus.OK)
  async getWorkspaceTokensList(
    @Request() request: PlatformRequest
  ): Promise<Array<ITokenProperties & { expired: boolean; project: { id: string; name: string } }>> {
    return this.tokenService.getWorkspaceTokensList(request.workspaceId)
  }

  @Delete(':tokenId')
  @ApiParam({
    name: 'tokenId',
    required: true,
    description: 'token identifier (UUIDv7)',
    type: 'string'
  })
  @ApiTags('Tokens')
  @ApiBearerAuth()
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  async deleteToken(
    @Param('tokenId') tokenId: string,
    @TransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<boolean | undefined> {
    const projectId = request.projectId
    return this.tokenService.deleteToken(tokenId, projectId, transaction)
  }
}
