import { CallHandler, ExecutionContext, Injectable, Logger, NestInterceptor } from '@nestjs/common'
import { Observable, from } from 'rxjs'
import { catchError, switchMap } from 'rxjs/operators'

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
          await internalTransaction?.close()
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

    // Register error/close handlers for aborted connections (cleanup on unexpected disconnects)
    if (res?.on) {
      res.on('error', () => void doCleanup(false))
      res.on('close', () => void doCleanup(false))
    }

    // Commit transaction WITHIN the RxJS pipe so it finishes before the response is sent.
    return next.handle().pipe(
      switchMap((value) => from(doCleanup(true).then(() => value))),
      catchError((error) =>
        from(
          doCleanup(false)
            .catch(() => {})
            .then(() => {
              throw error
            })
        )
      )
    )
  }
}
