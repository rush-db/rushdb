import { Transaction } from 'neo4j-driver'

import { AsyncLocalStorage } from 'async_hooks'

export interface TransactionContext {
  transaction: Transaction
}

export const transactionStorage = new AsyncLocalStorage<TransactionContext>()
