import { CallHandler, ExecutionContext, Injectable, Logger, NestInterceptor } from '@nestjs/common'
import { Observable } from 'rxjs'

import { NeogmaService } from '@/database/neogma/neogma.service'

@Injectable()
export class RequestCleanupInterceptor implements NestInterceptor {
  constructor(private readonly neogmaService: NeogmaService) {}

  intercept(context: ExecutionContext, next: CallHandler): Observable<any> {
    const http = context.switchToHttp()
    const request = http.getRequest<any>()
    const reply: any = http.getResponse()
    const res: any = reply?.raw ?? reply

    const raw: any = (request as any).raw ?? request

    // Snapshot references; these are set by middlewares prior to interceptors.
    const internalSession = raw.session ?? request.session
    const internalTransaction = raw.transaction ?? request.transaction
    const externalSession = raw.externalSession ?? request.externalSession
    const externalTransaction = raw.externalTransaction ?? request.externalTransaction

    let cleaned = false

    const doCleanup = async (success: boolean) => {
      if (cleaned) {
        return
      }

      cleaned = true

      try {
        // Internal transaction/session (from AuthMiddleware)
        if (internalTransaction?.isOpen?.()) {
          try {
            if (success) {
              await internalTransaction.commit()
            } else {
              await internalTransaction.rollback()
            }
          } catch (e) {
            Logger.error('[RequestCleanupInterceptor] Internal tx finalize error', e)
          }
        }
      } finally {
        try {
          await internalTransaction.close()
        } catch (e) {
          Logger.error('[RequestCleanupInterceptor] Internal tx finalize error', e)
        }
        try {
          if (internalSession) {
            await this.neogmaService.closeSession(internalSession)
          }
        } catch (e) {
          Logger.error('[RequestCleanupInterceptor] Internal session close error', e)
        }

        Logger.log('[RequestCleanupInterceptor] Internal session and tx close complete')
      }

      try {
        // External transaction/session (from SessionAttachMiddleware)
        if (externalTransaction?.isOpen?.()) {
          try {
            if (success) {
              await externalTransaction.commit()
            } else {
              await externalTransaction.rollback()
            }
          } catch (e) {
            Logger.error('[RequestCleanupInterceptor] External tx finalize error', e)
          }
        }
      } finally {
        try {
          await externalTransaction?.close?.()
        } catch (e) {
          Logger.error('[RequestCleanupInterceptor] External tx finalize error', e)
        }
        try {
          await externalSession?.close?.()
        } catch (e) {
          Logger.error('[RequestCleanupInterceptor] External session finalize error', e)
        }
        if (externalTransaction || externalSession) {
          Logger.log('[RequestCleanupInterceptor] External session and tx close complete')
        }
      }

      // Clear request-bound references to help GC
      try {
        raw.session = undefined
        raw.transaction = undefined
        raw.externalSession = undefined
        raw.externalTransaction = undefined
      } catch {
        /* empty */
      }
    }

    // Prefer to run cleanup after response has been sent to the client.
    if (res?.on) {
      res.on('finish', () => {
        const statusCode: number = res.statusCode ?? 500
        void doCleanup(statusCode < 400)
      })
      res.on('error', () => {
        void doCleanup(false)
      })
      // Fallback for aborted connections or streaming errors
      res.on('close', () => {
        void doCleanup(false)
      })
    } else {
      // As a very last resort, schedule cleanup (should rarely happen)
      setTimeout(() => void doCleanup(false), 5000).unref()
    }

    return next.handle()
  }
}
