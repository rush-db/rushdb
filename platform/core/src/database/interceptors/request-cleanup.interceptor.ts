import { CallHandler, ExecutionContext, Injectable, Logger, NestInterceptor } from '@nestjs/common'
import { Observable } from 'rxjs'

@Injectable()
export class RequestCleanupInterceptor implements NestInterceptor {
  intercept(context: ExecutionContext, next: CallHandler): Observable<any> {
    const http = context.switchToHttp()
    const request = http.getRequest<any>()
    const reply: any = http.getResponse()
    const res: any = reply?.raw ?? reply

    // Snapshot references; these are set by middlewares prior to interceptors.
    const internalSession = request.raw.session
    const internalTransaction = request.raw.transaction
    const externalSession = request.raw.externalSession
    const externalTransaction = request.raw.externalTransaction

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
        } catch {}
        try {
          if (internalSession) {
            await internalSession.close()
          }
        } catch {}
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
        } catch {}
        try {
          await externalSession?.close?.()
        } catch {}
      }

      // Clear request-bound references to help GC
      request.raw.session = undefined as any
      request.raw.transaction = undefined as any
      request.raw.externalSession = undefined as any
      request.raw.externalTransaction = undefined as any
    }

    // Prefer to run cleanup after response has been sent to the client.
    if (res?.on) {
      res.on('finish', () => {
        const statusCode: number = res.statusCode ?? 500
        void doCleanup(statusCode < 400)
      })
      // Fallback for aborted connections or streaming errors
      res.on('close', () => {
        void doCleanup(false)
      })
    }

    return next.handle()
  }
}
