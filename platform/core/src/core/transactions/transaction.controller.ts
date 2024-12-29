import {
  Body,
  Controller,
  Get,
  HttpCode,
  HttpStatus,
  Param,
  Post,
  Request,
  UseInterceptors
} from '@nestjs/common'
import { ApiBearerAuth, ApiTags } from '@nestjs/swagger'

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { PlatformRequest } from '@/common/types/request'
import { TransactionService } from '@/core/transactions/transaction.service'
import { TTransactionObject } from '@/core/transactions/transaction.types'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'

@Controller('tx')
@ApiTags('Transactions')
@UseInterceptors(TransformResponseInterceptor, NotFoundInterceptor)
export class TransactionController {
  constructor(private readonly transactionService: TransactionService) {}

  @Post()
  @ApiBearerAuth()
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  async create(
    @Body() config: { ttl: number },
    @Request() request: PlatformRequest
  ): Promise<Pick<TTransactionObject, 'id'>> {
    const projectId = request.projectId
    const { id } = this.transactionService.createTransaction(projectId, config)
    return { id }
  }

  @Get('/:txId')
  @ApiBearerAuth()
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  async read(@Param('txId') txId: string): Promise<Pick<TTransactionObject, 'id'>> {
    const { id } = this.transactionService.getTransaction(txId)
    return { id }
  }

  @Post('/:txId/rollback')
  @ApiBearerAuth()
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  async rollback(@Param('txId') txId: string): Promise<{ message: string }> {
    return this.transactionService.rollbackTransaction(txId)
  }

  @Post('/:txId/commit')
  @ApiBearerAuth()
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  async commit(@Param('txId') txId: string): Promise<{ message: string }> {
    return this.transactionService.commitTransaction(txId)
  }
}
