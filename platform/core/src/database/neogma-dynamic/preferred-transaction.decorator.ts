import { createParamDecorator, ExecutionContext } from '@nestjs/common'
import { Transaction } from 'neo4j-driver'

import { transactionStorage } from '@/core/transactions/transaction-context'

export const PreferredTransactionDecorator = createParamDecorator<unknown, ExecutionContext, Transaction>(
  (data, ctx) => {
    const request = ctx.switchToHttp().getRequest()
    const context = transactionStorage.getStore()
    return request.customTransaction || context?.transaction || request.transaction
  }
)
