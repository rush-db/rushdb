import { Injectable, NestMiddleware, Logger } from '@nestjs/common'
import { NextFunction, Response } from 'express'
import { Session, Transaction } from 'neo4j-driver'

import { PlatformRequest } from '@/common/types/request'
import { TransactionService } from '@/core/transactions/transaction.service'
import { dbContextStorage } from '@/database/db-context'

@Injectable()
export class SessionAndTransactionAttachMiddleware implements NestMiddleware {
  constructor(private readonly transactionService: TransactionService) {}

  async use(request: PlatformRequest, response: Response, next: NextFunction) {
    if (request.method === 'OPTIONS' || request.method === 'HEAD') {
      return next()
    }

    const dbContext = dbContextStorage.getStore()
    const externalDbConnection = dbContext.externalConnection
    const txId = <string>request.headers['x-transaction-id']

    const raw: any = (request as any).raw ?? request

    if (txId && !raw.userDefinedTransaction) {
      const userDefinedTransaction = this.transactionService.getTransaction(txId)
      raw.userDefinedTransaction = userDefinedTransaction.transaction
    }

    if (externalDbConnection) {
      let session: Session | null = null
      let transaction: Transaction | null = null

      try {
        if (raw.externalSession && raw.externalTransaction) {
          return next()
        }

        const { body, projectId, originalUrl, method, routerMethod, path, workspaceId } = raw

        session = externalDbConnection.driver?.session()
        transaction = session?.beginTransaction({
          timeout: 10_000,
          metadata: { body, projectId, originalUrl, method, routerMethod, path, workspaceId }
        })

        raw.externalSession = session
        raw.externalTransaction = transaction

        return next()
        //
      } catch (e) {
        Logger.error('[SessionAttachMiddleware] Error', e)
        try {
          await transaction?.close?.()
          await session?.close?.()
        } catch {
          Logger.error('[SessionAttachMiddleware] Error', e)
        } finally {
          raw.externalSession = undefined
          raw.externalTransaction = undefined
        }

        return next(e)
      }
    }
    return next()
  }
}
