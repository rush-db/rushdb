import { createParamDecorator, ExecutionContext } from '@nestjs/common'
import { transactionStorage } from '@/core/transactions/transaction-context'
import { Transaction } from 'neo4j-driver'

export const TransactionDecorator = createParamDecorator<unknown, ExecutionContext, Transaction>(
  (data, ctx) => {
    const request = ctx.switchToHttp().getRequest()
    const context = transactionStorage.getStore()
    return context?.transaction || request.transaction
  }
)
