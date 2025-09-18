import {
  ArgumentsHost,
  Catch,
  ExceptionFilter,
  HttpException,
  HttpStatus,
  Injectable,
  Logger
} from '@nestjs/common'
import { FastifyReply } from 'fastify'
import { Session, Transaction } from 'neo4j-driver'

import { isDevMode } from '@/common/utils/isDevMode'
import { toBoolean } from '@/common/utils/toBolean'
import { TransactionService } from '@/core/transactions/transaction.service'

@Injectable()
@Catch()
export class GlobalExceptionFilter implements ExceptionFilter {
  constructor(private readonly transactionService: TransactionService) {}

  async catch(exception: HttpException, host: ArgumentsHost) {
    const ctx = host.switchToHttp()
    const response = ctx.getResponse<FastifyReply>()
    const request = ctx.getRequest<any>()
    const isHttpException = exception instanceof HttpException

    const status = isHttpException ? exception.getStatus() : HttpStatus.INTERNAL_SERVER_ERROR

    const raw: any = (request as any).raw ?? request
    const internalSession = raw?.session
    const internalTransaction = raw?.transaction
    const externalSession = raw?.externalSession
    const externalTransaction = raw?.externalTransaction

    const userDefinedTransaction = request?.raw?.userDefinedTransaction
    const txId = <string>request.headers['x-transaction-id']

    isDevMode(() => {
      Logger.log('[ROLLBACK TRANSACTION]: Exception filter', JSON.stringify(exception))
    })

    console.log(exception)

    Logger.error(exception)

    // Helper to safely rollback (if open) and close a transaction
    const finalizeTx = async (label: string, tx?: Transaction) => {
      if (!tx) {
        return
      }
      try {
        await tx.rollback()
      } catch (e) {
        Logger.error(`[GlobalExceptionFilter] ${label} close error`, e as any)
      }
    }

    await finalizeTx('internal tx', internalTransaction as Transaction | undefined)
    await finalizeTx('external tx', externalTransaction as Transaction | undefined)

    const hasUserDefinedTransaction = toBoolean(userDefinedTransaction) && txId
    if (hasUserDefinedTransaction) {
      await this.transactionService.rollbackTransaction(txId)
    }

    // Close sessions last
    const finalizeSession = async (label: string, session?: Session) => {
      if (!session) {
        return
      }
      try {
        await session.close()
      } catch (e) {
        Logger.error(`[GlobalExceptionFilter] ${label} session close error`, e as any)
      }
    }

    await finalizeSession('internal', internalSession as Session | undefined)
    await finalizeSession('external', externalSession as Session | undefined)

    // Clear request-bound references to help GC and avoid double-closing
    try {
      raw.__rushdb_cleaned = true
      raw.transaction = undefined
      raw.session = undefined
      raw.externalTransaction = undefined
      raw.externalSession = undefined
    } catch {}

    response.status?.(status).send(isHttpException ? exception.getResponse() : {})
  }
}
