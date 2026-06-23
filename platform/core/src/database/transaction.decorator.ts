import { createParamDecorator, ExecutionContext } from '@nestjs/common'
import { Transaction } from 'neo4j-driver'

export const TransactionDecorator = createParamDecorator<unknown, Transaction>(
  (data, ctx: ExecutionContext) => {
    const request = ctx.switchToHttp().getRequest()
    return request.raw.transaction
  }
)
