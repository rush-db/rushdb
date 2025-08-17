import { Injectable, NestMiddleware, Logger } from '@nestjs/common'
import { NextFunction, Response } from 'express'
import { Session, Transaction } from 'neo4j-driver'

import { PlatformRequest } from '@/common/types/request'
import { dbContextStorage } from '@/database/db-context'

@Injectable()
export class SessionAttachMiddleware implements NestMiddleware {
  async use(request: PlatformRequest, response: Response, next: NextFunction) {
    const dbContext = dbContextStorage.getStore()
    const externalDbConnection = dbContext.externalConnection

    if (externalDbConnection) {
      let session: Session | null = null
      let transaction: Transaction | null = null

      try {
        session = externalDbConnection.driver?.session()
        transaction = session?.beginTransaction()

        request.externalSession = session
        request.externalTransaction = transaction

        return next()
      } catch (e) {
        Logger.error('[SessionAttachMiddleware] Error', e)
        try {
          await transaction?.close?.()
          await session?.close?.()
        } catch {
          Logger.error('[SessionAttachMiddleware] Error', e)
        } finally {
          request.externalSession = undefined
          request.externalTransaction = undefined
        }

        return next(e)
      }
    }

    next()
  }
}
