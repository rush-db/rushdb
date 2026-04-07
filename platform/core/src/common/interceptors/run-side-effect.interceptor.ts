import { CallHandler, ExecutionContext, Injectable, Logger, mixin, NestInterceptor } from '@nestjs/common'
import { Session, Transaction } from 'neo4j-driver'
import { Observable } from 'rxjs'
import { tap } from 'rxjs/operators'

import { isDevMode } from '@/common/utils/isDevMode'
import { ProjectService } from '@/dashboard/project/project.service'
import { dbContextStorage } from '@/database/db-context'
import { NeogmaService } from '@/database/neogma/neogma.service'

export enum ESideEffectType {
  RECOUNT_PROJECT_STRUCTURE = 'recountProjectNodes'
}

/**
 * Polls until a Neo4j transaction is no longer open (committed/rolled-back/closed).
 * Used to ensure side-effect reads happen after the primary request transaction commits.
 */
const waitForTransactionClose = (tx: Transaction | null | undefined, maxWaitMs = 15_000): Promise<void> => {
  if (!tx || !tx.isOpen()) {
    return Promise.resolve()
  }

  return new Promise((resolve, reject) => {
    const start = Date.now()
    const check = () => {
      if (!tx.isOpen()) {
        resolve()
      } else if (Date.now() - start > maxWaitMs) {
        reject(new Error('Timed out waiting for primary transaction to close'))
      } else {
        setTimeout(check, 50)
      }
    }
    check()
  })
}

export const RunSideEffectMixin = (sideEffects: ESideEffectType[]) => {
  @Injectable()
  class RunSideEffectInterceptor implements NestInterceptor {
    constructor(
      readonly neogmaService: NeogmaService,
      readonly projectService: ProjectService
    ) {}
    intercept(context: ExecutionContext, next: CallHandler): Observable<any> {
      const request = context.switchToHttp().getRequest()
      const raw: any = (request as any).raw ?? request

      const dbContext = dbContextStorage.getStore()
      const projectId = raw.projectId
      const externalDbConnection = dbContext.externalConnection

      return next.handle().pipe(
        tap(() => {
          // Snapshot references to the main request transactions before they get cleared
          const mainInternalTx: Transaction | undefined = raw.transaction
          const mainExternalTx: Transaction | undefined = raw.externalTransaction

          const runSideEffects = async () => {
            // Wait for the primary request transaction(s) to be committed by RequestCleanupInterceptor
            await waitForTransactionClose(mainInternalTx)
            if (mainExternalTx) {
              await waitForTransactionClose(mainExternalTx)
            }

            // Create fresh session/transaction for side-effect work (after primary commit)
            const session = this.neogmaService.createSession('run-side-effect')
            const transaction = session.beginTransaction({ timeout: 30_000 })

            let externalSession: Session | undefined
            let externalTransaction: Transaction | undefined

            if (externalDbConnection) {
              isDevMode(() =>
                Logger.debug(`External transaction created for project ${projectId} side effect runner`)
              )
              externalSession = externalDbConnection.driver?.session()
              externalTransaction = externalSession?.beginTransaction({ timeout: 30_000 })
            }

            try {
              const sideEffectsList = []

              const recountProjectStructureSideEffect = () => ({
                init: () =>
                  this.projectService.recomputeProjectNodes(projectId, transaction, externalTransaction)
              })

              sideEffects.forEach((sideEffectName) => {
                switch (sideEffectName) {
                  case ESideEffectType.RECOUNT_PROJECT_STRUCTURE:
                    sideEffectsList.push(recountProjectStructureSideEffect())
                }
              })

              await Promise.all(sideEffectsList.map((sideEffect) => sideEffect.init()))

              if (transaction.isOpen()) {
                await transaction.commit()
              }
              await this.neogmaService.closeSession(session, 'run-side-effect-interceptor')

              if (externalDbConnection && externalTransaction?.isOpen()) {
                isDevMode(() =>
                  Logger.log(`[COMMIT CUSTOM TRANSACTION]: Side effect runner for project ${projectId}`)
                )
                await externalTransaction.commit()
                await externalSession?.close()
              }
            } catch (error) {
              Logger.error(`[SideEffect ERROR]: project ${projectId}`, error)
              try {
                if (transaction.isOpen()) {
                  await transaction.rollback()
                }
              } catch {
                /* empty */
              }
              try {
                await this.neogmaService.closeSession(session, 'run-side-effect-interceptor')
              } catch {
                /* empty */
              }
              if (externalDbConnection) {
                try {
                  if (externalTransaction?.isOpen()) {
                    await externalTransaction.rollback()
                  }
                } catch {
                  /* empty */
                }
                try {
                  await externalSession?.close()
                } catch {
                  /* empty */
                }
              }
            }
          }

          // Fire-and-forget: side effects must not delay the HTTP response
          runSideEffects().catch((e) => {
            Logger.error(`[SideEffect runner ERROR]: project ${projectId}`, e)
          })
        })
      )
    }
  }

  return mixin(RunSideEffectInterceptor)
}
