import { Injectable, NestInterceptor, ExecutionContext, CallHandler, Logger } from '@nestjs/common'
import { Session, Transaction } from 'neo4j-driver'
import { concatMap, Observable } from 'rxjs'
import { catchError } from 'rxjs/operators'

import { isDevMode } from '@/common/utils/isDevMode'
import { TransactionService } from '@/core/transactions/transaction.service'
import { NeogmaService } from '@/database/neogma/neogma.service'

@Injectable()
export class NeogmaTransactionInterceptor implements NestInterceptor {
  constructor(
    private readonly neogmaService: NeogmaService,
    private readonly transactionService: TransactionService
  ) {}

  intercept(context: ExecutionContext, next: CallHandler): Observable<any> {
    const request = context.switchToHttp().getRequest()

    // Check if transaction already in Request Object (as it may be added from GlobalAuthGuard)
    const txId = request.headers['x-transaction-id']

    let transaction: Transaction
    let session: Session

    if (!txId && !request.transaction) {
      session = this.neogmaService.createSession()
      transaction = session.beginTransaction()
    } else {
      const clientTransaction = this.transactionService.getTransaction(txId)
      transaction = clientTransaction?.transaction ?? request.transaction
      session = clientTransaction?.session ?? request.session
    }

    // @TODO: Seems like if here is redundant (make discovery on this matter)
    if (!request.transaction) {
      request.transaction = transaction
      request.session = session
    }

    return next.handle().pipe(
      concatMap(async (data) => {
        if (!txId && transaction.isOpen()) {
          await transaction.commit()
          await this.neogmaService.closeSession(session, 'neogma-transaction-interceptor')
          isDevMode(() => Logger.log('[COMMIT TRANSACTION]'))
        }
        return data
      }),
      catchError(async (error) => {
        await transaction.rollback()
        await this.neogmaService.closeSession(session, 'neogma-transaction-interceptor')

        isDevMode(() => Logger.log('[ROLLBACK TRANSACTION]: Neogma interceptor', error))

        throw error
      })
    )
  }
}
