import { Injectable, NestMiddleware, Logger } from '@nestjs/common'
import { NextFunction, Response } from 'express'
import { Session, Transaction } from 'neo4j-driver'

import { PlatformRequest } from '@/common/types/request'
import { TransactionService } from '@/core/transactions/transaction.service'
import { TTransactionObject } from '@/core/transactions/transaction.types'
import { dbContextStorage } from '@/database/db-context'

@Injectable()
export class SessionAndTransactionAttachMiddleware implements NestMiddleware {
  constructor(private readonly transactionService: TransactionService) {}

  async use(request: PlatformRequest, response: Response, next: NextFunction) {
    const dbContext = dbContextStorage.getStore()
    const externalDbConnection = dbContext.externalConnection
    const txId = <string>request.headers['x-transaction-id']

    let userDefinedTransaction: TTransactionObject
    if (txId) {
      userDefinedTransaction = this.transactionService.getTransaction(txId)
    }
    if (userDefinedTransaction) {
      request.userDefinedTransaction = userDefinedTransaction.transaction
    }

    if (externalDbConnection) {
      let session: Session | null = null
      let transaction: Transaction | null = null

      try {
        session = externalDbConnection.driver?.session()
        transaction = session?.beginTransaction()

        const raw: any = (request as any).raw ?? request
        raw.externalSession = session
        raw.externalTransaction = transaction

        return next()
      } catch (e) {
        Logger.error('[SessionAttachMiddleware] Error', e)
        try {
          await transaction?.close?.()
          await session?.close?.()
        } catch {
          Logger.error('[SessionAttachMiddleware] Error', e)
        } finally {
          const raw: any = (request as any).raw ?? request
          raw.externalSession = undefined
          raw.externalTransaction = undefined
        }

        return next(e)
      }
    }

    next()
  }
}
