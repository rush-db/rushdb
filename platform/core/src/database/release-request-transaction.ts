import { Logger } from '@nestjs/common'

/**
 * Commits and disposes the request-scoped Neo4j transaction/session(s) attached by
 * AuthMiddleware / SessionAndTransactionAttachMiddleware, before the handler runs its
 * long-lived work.
 *
 * Endpoints that spend most of their time outside the graph (LLM round-trips, cached
 * schema reads, background-fanned recalculations) otherwise hold the auth-attached
 * transaction open while it idles; its server-side time budget keeps ticking on
 * wall-clock, so the commit in RequestCleanupInterceptor later fails with
 * TransactionTimedOut and the whole request 408s despite doing no graph work.
 * Releasing early decouples such endpoints from the transaction budget entirely.
 *
 * User-defined transactions (x-transaction-id) are left untouched — they are owned by
 * TransactionService. RequestCleanupInterceptor's later pass no-ops on the closed
 * transaction (isOpen() is false) and closing an already-closed session is safe.
 */
export const releaseRequestTransaction = async (request: any): Promise<void> => {
  const raw: any = request?.raw ?? request
  if (!raw) {
    return
  }

  const { transaction, session, externalTransaction, externalSession } = raw
  raw.transaction = undefined
  raw.session = undefined
  raw.externalTransaction = undefined
  raw.externalSession = undefined

  const finalize = async (tx: any, txSession: any, label: string) => {
    try {
      if (tx?.isOpen?.()) {
        // Nothing has been written on this transaction before the handler; commit and
        // rollback are equivalent — commit keeps parity with the success path.
        await tx.commit()
      }
    } catch (error) {
      Logger.error(`[releaseRequestTransaction] ${label} commit failed`, error as any)
    } finally {
      try {
        await tx?.close?.()
      } catch {
        /* empty */
      }
      try {
        await txSession?.close?.()
      } catch {
        /* empty */
      }
    }
  }

  await finalize(transaction, session, 'internal')
  await finalize(externalTransaction, externalSession, 'external')
}
