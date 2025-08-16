import { createParamDecorator, ExecutionContext, BadRequestException, Logger } from '@nestjs/common'
import { Transaction } from 'neo4j-driver'

export const TransactionDecorator = createParamDecorator<unknown, ExecutionContext, Transaction>(
  (data, ctx) => {
    const request = ctx.switchToHttp().getRequest()
    return request.raw.transaction
  }
)
