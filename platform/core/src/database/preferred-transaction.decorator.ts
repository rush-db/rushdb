import { BadRequestException, createParamDecorator, ExecutionContext, Logger } from '@nestjs/common'
import { Transaction } from 'neo4j-driver'

export const PreferredTransactionDecorator = createParamDecorator<unknown, ExecutionContext, Transaction>(
  (data, ctx) => {
    const request = ctx.switchToHttp().getRequest()

    if (!request.raw.userDefinedTransaction && !request.raw.transaction && !request.raw.externalTransaction) {
      Logger.error('[NO TRANSACTION]')
      throw new BadRequestException('Transaction not found')
    }

    // user defined txn has highest priority
    return request.raw.userDefinedTransaction ?? request.raw.externalTransaction ?? request.raw.transaction
  }
)
