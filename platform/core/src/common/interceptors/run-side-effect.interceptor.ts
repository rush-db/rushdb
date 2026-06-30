import {
  CallHandler,
  ExecutionContext,
  Injectable,
  Logger,
  mixin,
  NestInterceptor,
  Optional
} from '@nestjs/common'
import { Session, Transaction } from 'neo4j-driver'
import { Observable } from 'rxjs'
import { tap } from 'rxjs/operators'

import { isDevMode } from '@/common/utils/isDevMode'
import { AiService } from '@/core/ai/ai.service'
import { RelationshipPatternsService } from '@/core/relationship-patterns/relationship-patterns.service'
import { ProjectService } from '@/dashboard/project/project.service'
import { dbContextStorage } from '@/database/db-context'
import { NeogmaService } from '@/database/neogma/neogma.service'

export enum ESideEffectType {
  RECOUNT_PROJECT_STRUCTURE = 'recountProjectNodes',
  RECALCULATE_SCHEMA_CACHE = 'recalculateSchemaCache',
  RELATIONSHIP_AUTOMATION_AFTER_WRITE = 'relationshipAutomationAfterWrite'
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
      readonly projectService: ProjectService,
      @Optional()
      readonly relationshipPatternsService?: RelationshipPatternsService,
      @Optional()
      readonly aiService?: AiService
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

            // Whether the schema cache must be refreshed once, AFTER the write
            // transaction commits. The recompute is a heavy full-graph scan, so it runs
            // on its own transaction (see recomputeSchema below) rather than sharing
            // the write transaction's timeout budget — which previously caused
            // TransactionTimedOutClientConfiguration errors.
            let schemaRecomputeNeeded = sideEffects.includes(ESideEffectType.RECALCULATE_SCHEMA_CACHE)

            try {
              const sideEffectsList = []

              const recountProjectStructureSideEffect = () => ({
                init: () =>
                  this.projectService.recomputeProjectNodes(projectId, transaction, externalTransaction)
              })

              const relationshipAutomationSideEffect = () => ({
                init: async () => {
                  if (!this.relationshipPatternsService) {
                    return
                  }
                  await this.relationshipPatternsService.markAfterWrite(projectId)
                  // applyApprovedPatterns returns the number of relationships materialized;
                  // only a non-zero count changes the graph and warrants a recompute.
                  const applied = await this.relationshipPatternsService.applyApprovedPatterns(
                    projectId,
                    transaction
                  )
                  if (applied > 0) {
                    schemaRecomputeNeeded = true
                  }
                }
              })

              sideEffects.forEach((sideEffectName) => {
                switch (sideEffectName) {
                  case ESideEffectType.RECOUNT_PROJECT_STRUCTURE:
                    sideEffectsList.push(recountProjectStructureSideEffect())
                    break
                  case ESideEffectType.RECALCULATE_SCHEMA_CACHE:
                    // Handled post-commit via schemaRecomputeNeeded — no inline work.
                    break
                  case ESideEffectType.RELATIONSHIP_AUTOMATION_AFTER_WRITE:
                    sideEffectsList.push(relationshipAutomationSideEffect())
                    break
                }
              })

              for (const sideEffect of sideEffectsList) {
                await sideEffect.init()
              }

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

              // Recompute the schema cache only after the write transaction has
              // committed, on a dedicated transaction so it sees all writes and never
              // competes with them for the timeout budget.
              if (schemaRecomputeNeeded && this.aiService) {
                try {
                  await recomputeSchemaInOwnTransaction(
                    this.aiService,
                    this.neogmaService,
                    projectId,
                    externalDbConnection
                  )
                } catch (schemaError) {
                  Logger.error(`[SideEffect schema recompute ERROR]: project ${projectId}`, schemaError)
                }
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

/**
 * Forces an schema cache recompute on a dedicated session/transaction. Run only after
 * the primary side-effect transaction has committed, so the heavy full-graph scan sees
 * every write yet never shares the write transaction's timeout budget.
 */
const recomputeSchemaInOwnTransaction = async (
  aiService: AiService,
  neogmaService: NeogmaService,
  projectId: string,
  externalDbConnection: any
): Promise<void> => {
  if (externalDbConnection) {
    const externalSession: Session | undefined = externalDbConnection.driver?.session()
    const externalTransaction = externalSession?.beginTransaction({ timeout: 30_000 })
    if (!externalTransaction) {
      return
    }
    try {
      await aiService.getSchema({ projectId, force: true, transaction: externalTransaction })
      if (externalTransaction.isOpen()) {
        await externalTransaction.commit()
      }
    } catch (error) {
      if (externalTransaction.isOpen()) {
        try {
          await externalTransaction.rollback()
        } catch {
          /* empty */
        }
      }
      throw error
    } finally {
      await externalSession?.close()
    }
    return
  }

  const session = neogmaService.createSession('run-side-effect-schema')
  const transaction = session.beginTransaction({ timeout: 30_000 })
  try {
    await aiService.getSchema({ projectId, force: true, transaction })
    if (transaction.isOpen()) {
      await transaction.commit()
    }
  } catch (error) {
    if (transaction.isOpen()) {
      try {
        await transaction.rollback()
      } catch {
        /* empty */
      }
    }
    throw error
  } finally {
    await neogmaService.closeSession(session, 'run-side-effect-schema')
  }
}
