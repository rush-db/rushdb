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

    // Post-commit hooks: work that must observe the request's committed writes (e.g. the
    // side-effect runner recounting records / recomputing the schema). Registered by inner
    // interceptors during the response phase and drained here, strictly AFTER the request
    // transaction is committed — so no polling for the tx to close is needed.
    if (!Array.isArray(raw.postCommitHooks)) {
      raw.postCommitHooks = []
    }

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

      // Drain post-commit hooks only on a successful request, now that the request
      // transaction is committed and its writes are visible. Fire-and-forget: hooks
      // must not delay the HTTP response (a schema recompute can take seconds).
      if (success && Array.isArray(raw.postCommitHooks) && raw.postCommitHooks.length > 0) {
        const hooks: Array<() => Promise<void> | void> = raw.postCommitHooks
        raw.postCommitHooks = []
        for (const hook of hooks) {
          Promise.resolve()
            .then(hook)
            .catch((e) => Logger.error('[RequestCleanupInterceptor] post-commit hook error', e))
        }
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
