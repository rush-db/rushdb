import { AsyncLocalStorage } from 'async_hooks'
import { Transaction } from 'neo4j-driver'

export interface TransactionContext {
  transaction: Transaction
}

export const transactionStorage = new AsyncLocalStorage<TransactionContext>()
