import {
  Body,
  Controller,
  Delete,
  Get,
  HttpCode,
  HttpStatus,
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
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { ChangeCorsInterceptor } from '@/dashboard/common/interceptors/change-cors.interceptor'
import { CreateTokenDto } from '@/dashboard/token/dto/create-token.dto'
import { ITokenProperties } from '@/dashboard/token/model/token.interface'
import { TokenService } from '@/dashboard/token/token.service'
import { NeogmaDataInterceptor } from '@/database/neogma/neogma-data.interceptor'
import { NeogmaTransactionInterceptor } from '@/database/neogma/neogma-transaction.interceptor'
import { TransactionDecorator } from '@/database/neogma/transaction.decorator'

@Controller('tokens')
@ApiExcludeController()
@UseInterceptors(
  TransformResponseInterceptor,
  NotFoundInterceptor,
  NeogmaDataInterceptor,
  NeogmaTransactionInterceptor,
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
    @TransactionDecorator() transaction: Transaction
  ): Promise<boolean | undefined> {
    // @TODO: Check ownership
    return this.tokenService.deleteToken(tokenId, transaction)
  }
}
