import { Injectable, Logger } from '@nestjs/common'
import { uuidv7 } from 'uuidv7'

import { getCurrentISO } from '@/common/utils/getCurrentISO'
import { isDevMode } from '@/common/utils/isDevMode'
import { TTransactionObject } from '@/core/transactions/transaction.types'
import { CompositeNeogmaService } from '@/database/neogma-dynamic/composite-neogma.service'

@Injectable()
export class TransactionService {
  constructor(private readonly compositeNeogmaService: CompositeNeogmaService) {}
  private transactions: Map<string, TTransactionObject> = new Map()

  createTransaction(projectId: string, config?: { ttl?: number }): TTransactionObject {
    const id = uuidv7()

    const session = this.compositeNeogmaService.createSession()
    const rushDBTransaction: TTransactionObject = {
      id,
      projectId,
      startTime: getCurrentISO(),
      session,
      transaction: session.beginTransaction()
    }

    this.transactions.set(id, rushDBTransaction)
    this.setTransactionTimeout(id, config.ttl)

    return rushDBTransaction
  }

  async commitTransaction(id: string) {
    if (this.transactions.has(id)) {
      const rushDBTransaction = this.transactions.get(id)
      await rushDBTransaction.transaction.commit()
      await this.clean(id)
    }
    return { message: `Transaction (${id}) has been successfully committed.` }
  }

  private async clean(id: string) {
    if (this.transactions.has(id)) {
      const rushDBTransaction = this.transactions.get(id)

      await rushDBTransaction.transaction.close()
      await this.compositeNeogmaService.closeSession(rushDBTransaction.session)

      this.transactions.delete(id)
    }
  }

  async rollbackTransaction(id: string) {
    if (this.transactions.has(id)) {
      const rushDBTransaction = this.transactions.get(id)

      if (rushDBTransaction.transaction.isOpen()) {
        isDevMode(() => Logger.log('[ROLLBACK TRANSACTION]: Transaction service'))

        await rushDBTransaction.transaction.rollback()
      }
      await this.clean(id)
    }

    return { message: `Transaction (${id}) has been rolled back.` }
  }

  getTransaction(id: string): TTransactionObject {
    if (!this.transactions.has(id)) {
      return
      // throw new NotFoundException(`Transaction with ID ${id} not found`);
    }
    return this.transactions.get(id)
  }

  private setTransactionTimeout(id: string, ttl = 5000) {
    setTimeout(() => {
      if (this.transactions.has(id)) {
        this.rollbackTransaction(id)
      }
    }, ttl)
  }
}
