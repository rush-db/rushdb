import { Injectable, Logger, NotFoundException } from '@nestjs/common'
import { uuidv7 } from 'uuidv7'

import { getCurrentISO } from '@/common/utils/getCurrentISO'
import { isDevMode } from '@/common/utils/isDevMode'
import { TTransactionObject } from '@/core/transactions/transaction.types'
import { dbContextStorage } from '@/database/db-context'

const MAX_TTL = 30000 // 30s
const DEFAULT_TTL = 5000 // 5s

@Injectable()
export class TransactionService {
  private transactions: Map<string, TTransactionObject> = new Map()

  createTransaction(projectId: string, config?: { ttl?: number }): TTransactionObject {
    const id = uuidv7()

    const dbContext = dbContextStorage.getStore()
    const connection = dbContext?.externalConnection ?? dbContext?.connection

    const session = connection.driver.session()
    const rushDBTransaction: TTransactionObject = {
      id,
      projectId,
      startTime: getCurrentISO(),
      session,
      transaction: session.beginTransaction({
        timeout: config?.ttl ?? 10_000,
        metadata: {
          id,
          projectId
        }
      }),
      timeout: this.setTransactionTimeout(id, config?.ttl),
      status: 'open'
    }
    this.transactions.set(id, rushDBTransaction)

    return rushDBTransaction
  }

  private async clean(id: string) {
    if (this.transactions.has(id)) {
      const rushDBTransaction = this.transactions.get(id)

      await rushDBTransaction.transaction.close()
      await rushDBTransaction.session.close()

      this.transactions.delete(id)
    }
  }

  getTransaction(id: string): TTransactionObject {
    if (!this.transactions.has(id)) {
      throw new NotFoundException(`Transaction with ID ${id} not found`)
    }
    return this.transactions.get(id)
  }

  async commitTransaction(id: string) {
    const tx = this.transactions.get(id)

    tx.status = 'committing'

    try {
      await tx.transaction.commit()
    } catch (e) {
      Logger.error('[TRANSACTION SERVICE]: Commit error: ', e)
    } finally {
      await this.clean(id)
    }

    return { message: `Transaction (${id}) has been successfully committed.` }
  }

  async rollbackTransaction(id: string) {
    const tx = this.transactions.get(id)
    if (!tx) {
      return { message: `Transaction (${id}) not found (already closed?)` }
    }

    if (tx.status !== 'open') {
      return { message: `Transaction (${id}) is ${tx.status}.` }
    }
    tx.status = 'rolling_back'

    try {
      if (tx.transaction.isOpen()) {
        isDevMode(() => Logger.log('[ROLLBACK TRANSACTION]: Transaction service'))
        await tx.transaction.rollback()
      }
    } catch (error) {
      Logger.error('[ROLLBACK TRANSACTION]: Transaction service error: ', error)
    } finally {
      await this.clean(id)
    }

    return { message: `Transaction (${id}) has been rolled back.` }
  }

  private setTransactionTimeout(id: string, ttl = DEFAULT_TTL) {
    const timeoutMs = Math.min(ttl, MAX_TTL)

    return setTimeout(async () => {
      Logger.warn(`Transaction timeout: ${id}: ${timeoutMs}ms`)

      const tx = this.transactions.get(id)
      if (!tx) {
        return
      }
      tx.status = 'expired'
      await this.rollbackTransaction(id)
    }, timeoutMs).unref()
  }
}
