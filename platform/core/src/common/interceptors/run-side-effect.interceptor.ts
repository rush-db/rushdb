import { CallHandler, ExecutionContext, Injectable, Logger, mixin, NestInterceptor } from '@nestjs/common'
import { Session, Transaction } from 'neo4j-driver'
import { Observable } from 'rxjs'
import { catchError, tap } from 'rxjs/operators'

import { isDevMode } from '@/common/utils/isDevMode'
import { ProjectService } from '@/dashboard/project/project.service'
import { dbContextStorage } from '@/database/db-context'
import { NeogmaService } from '@/database/neogma/neogma.service'

export enum ESideEffectType {
  RECOUNT_PROJECT_STRUCTURE = 'recountProjectNodes'
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

      const session = this.neogmaService.createSession('run-side-effect')
      const transaction = session.beginTransaction()

      let externalSession: Session
      let externalTransaction: Transaction

      const dbContext = dbContextStorage.getStore()

      const projectId = request?.raw?.projectId
      const externalDbConnection = dbContext.externalConnection

      if (externalDbConnection) {
        isDevMode(() =>
          Logger.debug(`External transaction created for project ${projectId} side effect runner`)
        )
        externalSession = externalDbConnection.driver?.session()
        externalTransaction = externalSession?.beginTransaction()
      }

      return next.handle().pipe(
        tap(async () => {
          // @TODO: Figure out how to run all of this after previous transaction is closed and successfully written
          // @FYI: Just hacky way to ensure that this Effect will be running a second after previous transaction is closed
          setTimeout(async () => {
            const sideEffectsList = []

            const recountProjectStructureSideEffect = () => {
              const init = async () => {
                return this.projectService.recomputeProjectNodes(projectId, transaction, externalTransaction)
              }

              return {
                init
              }
            }

            sideEffects.map((sideEffectName) => {
              switch (sideEffectName) {
                case ESideEffectType.RECOUNT_PROJECT_STRUCTURE:
                  sideEffectsList.push(recountProjectStructureSideEffect())
              }
            })

            const initialisedSideEffects = sideEffectsList.map(async (sideEffect) => {
              await sideEffect.init()
            })

            await Promise.all(initialisedSideEffects)
            if (transaction.isOpen()) {
              await transaction.commit()
              await this.neogmaService.closeSession(session, 'run-side-effect-interceptor')
            }

            if (externalDbConnection && externalTransaction?.isOpen()) {
              isDevMode(() =>
                Logger.log(`[COMMIT CUSTOM TRANSACTION]: Side effect runner for project ${projectId}`)
              )
              await externalTransaction.commit()
              await externalSession.close()
            }
          }, 1000)
        }),
        catchError(async (error) => {
          isDevMode(() => Logger.log(`[ROLLBACK TRANSACTION]: Side effect runner for project ${projectId}`))
          await transaction.rollback()
          await this.neogmaService.closeSession(session, 'run-side-effect-interceptor')

          if (externalDbConnection) {
            isDevMode(() =>
              Logger.log(`[ROLLBACK CUSTOM TRANSACTION]: Side effect runner for project ${projectId}`)
            )
            await externalTransaction.rollback()
            await externalSession.close()
          }
          throw error
        })
      )
    }
  }

  return mixin(RunSideEffectInterceptor)
}
