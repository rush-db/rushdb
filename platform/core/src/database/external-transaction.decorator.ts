import { BadRequestException, createParamDecorator, ExecutionContext, Logger } from '@nestjs/common'
import { Transaction } from 'neo4j-driver'

export const ExternalTransactionDecorator = createParamDecorator<unknown, ExecutionContext, Transaction>(
  (data, ctx) => {
    const request = ctx.switchToHttp().getRequest()

    if (
      !request.raw.externalSession &&
      !request.raw.userDefinedTransaction &&
      !request.raw.externalTransaction
    ) {
      Logger.error('[NO EXTERNAL TRANSACTION]')
      throw new BadRequestException('External transaction not found')
    }

    // user defined txn has highest priority
    return request.raw.userDefinedTransaction ?? request.raw.externalTransaction
  }
)
