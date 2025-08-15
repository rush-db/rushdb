import {
  ArgumentsHost,
  Catch,
  ExceptionFilter,
  HttpException,
  HttpStatus,
  Injectable,
  Logger
} from '@nestjs/common'
import { FastifyReply, FastifyRequest } from 'fastify'
import { Session, Transaction } from 'neo4j-driver'

import { isDevMode } from '@/common/utils/isDevMode'
import { NeogmaService } from '@/database/neogma/neogma.service'

@Injectable()
@Catch()
export class GlobalExceptionFilter implements ExceptionFilter {
  constructor(private readonly neogmaService: NeogmaService) {}

  async catch(exception: HttpException, host: ArgumentsHost) {
    const ctx = host.switchToHttp()
    const response = ctx.getResponse<FastifyReply>()
    const request = ctx.getRequest<any>()
    const isHttpException = exception instanceof HttpException

    const status = isHttpException ? exception.getStatus() : HttpStatus.INTERNAL_SERVER_ERROR

    const internalSession = request?.raw?.session
    const internalTransaction = request?.raw?.transaction
    const externalSession = request?.raw?.externalSession
    const externalTransaction = request?.raw?.externalTransaction

    isDevMode(() => {
      Logger.log('[ROLLBACK TRANSACTION]: Exception filter', JSON.stringify(exception))
      console.log(exception)
    })

    // Helper to safely rollback (if open) and close a transaction
    const finalizeTx = async (label: string, tx?: Transaction) => {
      if (!tx) {
        return
      }
      try {
        if (typeof (tx as any).isOpen === 'function' && (tx as any).isOpen()) {
          try {
            await tx.rollback()
          } catch (e) {
            Logger.error(`[GlobalExceptionFilter] ${label} rollback error`, e as any)
          }
        }
      } finally {
        try {
          await tx.close()
        } catch (e) {
          Logger.error(`[GlobalExceptionFilter] ${label} close error`, e as any)
        }
      }
    }

    await finalizeTx('internal tx', internalTransaction as Transaction | undefined)
    await finalizeTx('external tx', externalTransaction as Transaction | undefined)

    // Close sessions last
    const finalizeSession = async (label: string, s?: Session) => {
      if (!s) {
        return
      }
      try {
        await this.neogmaService.closeSession(s, ` global-exception-filter (${label})`)
      } catch (e) {
        Logger.error(`[GlobalExceptionFilter] ${label} session close error`, e as any)
      }
    }

    await finalizeSession('internal', internalSession as Session | undefined)
    await finalizeSession('external', externalSession as Session | undefined)

    // Clear request-bound references to help GC and avoid double-closing
    try {
      const raw: any = (request as any).raw ?? (request as any)
      raw.transaction = undefined
      raw.session = undefined
      raw.externalTransaction = undefined
      raw.externalSession = undefined
    } catch {}

    response.status?.(status).send(isHttpException ? exception.getResponse() : {})
  }
}
