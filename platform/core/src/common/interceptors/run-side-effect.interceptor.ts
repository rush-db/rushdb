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
import { DEFAULT_TRANSACTION_TIMEOUT_MS } from '@/database/transaction.constants'

export enum ESideEffectType {
  RECOUNT_PROJECT_STRUCTURE = 'recountProjectNodes',
  RECALCULATE_SCHEMA_CACHE = 'recalculateSchemaCache',
  RELATIONSHIP_AUTOMATION_AFTER_WRITE = 'relationshipAutomationAfterWrite'
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
      const externalDbConnection = dbContext?.externalConnection

      return next.handle().pipe(
        tap(() => {
          // Snapshot the user-defined transaction before request references are cleared.
          const userDefinedTx: Transaction | undefined = raw.userDefinedTransaction

          const runSideEffects = () =>
            this.executeSideEffects({ projectId, userDefinedTx, externalDbConnection })

          // Prefer the RequestCleanupInterceptor hook: it drains registered hooks strictly
          // AFTER the request transaction is committed, so side effects always observe the
          // write — no polling for the tx to close, no 15s abort window. When the hook is
          // unavailable (e.g. the CLI/backup restore path that invokes this interceptor with
          // a mock request and commits its own transaction beforehand), run inline instead.
          if (Array.isArray(raw.postCommitHooks)) {
            raw.postCommitHooks.push(() =>
              runSideEffects().catch((e) => {
                Logger.error(`[SideEffect runner ERROR]: project ${projectId}`, e)
              })
            )
          } else {
            runSideEffects().catch((e) => {
              Logger.error(`[SideEffect runner ERROR]: project ${projectId}`, e)
            })
          }
        })
      )
    }

    /**
     * Runs the requested side effects after the request's writes are committed and visible.
     *
     * Ordering matters: the graph-mutating steps (recount, applying approved relationship
     * patterns) run and commit first; the schema cache is recomputed next so it reflects
     * those writes; and relationship analysis is queued LAST, so the analysis reads the
     * freshly recomputed schema instead of a stale/empty cache. Each step is isolated in its
     * own try/catch so one failure never cascades into skipping the others.
     */
    async executeSideEffects({
      projectId,
      userDefinedTx,
      externalDbConnection
    }: {
      projectId: string
      userDefinedTx: Transaction | undefined
      externalDbConnection: any
    }): Promise<void> {
      // Writes made through a user-defined transaction (x-transaction-id) are not committed
      // yet when the response is sent — recounting/recomputing now would read pre-commit
      // state (e.g. a fresh import counting 0 records). Defer: the same side effects run on
      // POST /tx/:txId/commit once the data is visible.
      if (userDefinedTx?.isOpen?.()) {
        isDevMode(() =>
          Logger.debug(
            `Side effects deferred to transaction commit for project ${projectId} (open user-defined transaction)`
          )
        )
        return
      }

      const wantsRecount = sideEffects.includes(ESideEffectType.RECOUNT_PROJECT_STRUCTURE)
      const wantsAutomation = sideEffects.includes(ESideEffectType.RELATIONSHIP_AUTOMATION_AFTER_WRITE)
      // The schema recompute is a heavy full-graph scan; it runs on getSchema's own
      // short-lived sessions rather than sharing a write transaction's timeout budget.
      let schemaRecomputeNeeded = sideEffects.includes(ESideEffectType.RECALCULATE_SCHEMA_CACHE)

      // ── Phase 1: graph-mutating steps on a fresh side-effect transaction ──────────
      if (wantsRecount || wantsAutomation) {
        const session = this.neogmaService.createSession('run-side-effect')
        const transaction = session.beginTransaction({ timeout: DEFAULT_TRANSACTION_TIMEOUT_MS })

        let externalSession: Session | undefined
        let externalTransaction: Transaction | undefined

        if (externalDbConnection) {
          isDevMode(() =>
            Logger.debug(`External transaction created for project ${projectId} side effect runner`)
          )
          externalSession = externalDbConnection.driver?.session()
          externalTransaction = externalSession?.beginTransaction({
            timeout: DEFAULT_TRANSACTION_TIMEOUT_MS
          })
        }

        try {
          if (wantsRecount) {
            try {
              await this.projectService.recomputeProjectNodes(projectId, transaction, externalTransaction)
            } catch (error) {
              Logger.error(`[SideEffect recount ERROR]: project ${projectId}`, error)
            }
          }

          if (wantsAutomation && this.relationshipPatternsService) {
            try {
              // applyApprovedPatterns returns the number of relationships materialized;
              // only a non-zero count changes the graph and warrants a recompute.
              const applied = await this.relationshipPatternsService.applyApprovedPatterns(
                projectId,
                transaction
              )
              if (applied > 0) {
                schemaRecomputeNeeded = true
              }
            } catch (error) {
              Logger.error(`[SideEffect relationship apply ERROR]: project ${projectId}`, error)
            }
          }

          if (transaction.isOpen()) {
            await transaction.commit()
          }
          if (externalDbConnection && externalTransaction?.isOpen()) {
            isDevMode(() =>
              Logger.log(`[COMMIT CUSTOM TRANSACTION]: Side effect runner for project ${projectId}`)
            )
            await externalTransaction.commit()
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
            if (externalTransaction?.isOpen()) {
              await externalTransaction.rollback()
            }
          } catch {
            /* empty */
          }
        } finally {
          try {
            await this.neogmaService.closeSession(session, 'run-side-effect-interceptor')
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

      // ── Phase 2: recompute the schema cache so it reflects the committed writes ────
      if (schemaRecomputeNeeded && this.aiService) {
        try {
          await this.aiService.getSchema({ projectId, force: true })
        } catch (schemaError) {
          Logger.error(`[SideEffect schema recompute ERROR]: project ${projectId}`, schemaError)
        }
      }

      // ── Phase 3: queue relationship analysis LAST ─────────────────────────────────
      // markAfterWrite may kick off an immediate analysis that loads the schema WITHOUT
      // force, trusting the cache to be fresh. That only holds because Phase 2 just
      // recomputed it — running this before the recompute made the analysis read a
      // stale/empty schema and store zero suggestions until the next write.
      if (wantsAutomation && this.relationshipPatternsService) {
        try {
          await this.relationshipPatternsService.markAfterWrite(projectId)
        } catch (error) {
          Logger.error(`[SideEffect relationship analysis ERROR]: project ${projectId}`, error)
        }
      }
    }
  }

  return mixin(RunSideEffectInterceptor)
}
