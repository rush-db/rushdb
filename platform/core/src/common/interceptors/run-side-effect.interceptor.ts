import { CallHandler, ExecutionContext, Injectable, mixin, NestInterceptor } from '@nestjs/common'
import { Observable } from 'rxjs'
import { catchError, tap } from 'rxjs/operators'

import { ProjectService } from '@/dashboard/project/project.service'
import { NeogmaService } from '@/database/neogma/neogma.service'

export enum ESideEffectType {
  RECOUNT_PROJECT_STRUCTURE = 'recountProjectNodes'
}

export const RunSideEffectMixin = (sideEffects: ESideEffectType[]) => {
  @Injectable()
  class RunSideEffectInterceptor implements NestInterceptor {
    constructor(readonly neogmaService: NeogmaService, readonly projectService: ProjectService) {}
    intercept(context: ExecutionContext, next: CallHandler): Observable<any> {
      const { projectId } = context.switchToHttp().getRequest()
      const session = this.neogmaService.createSession()
      const transaction = session.beginTransaction()
      return next.handle().pipe(
        tap(async () => {
          // @TODO: Figure out how to run all of this after previous transaction is closed and successfully written
          // @FYI: Just hacky way to ensure that this Effect will be running a second after previous transaction is closed
          setTimeout(async () => {
            const sideEffectsList = []

            const recountProjectStructureSideEffect = () => {
              const targetId = projectId

              const init = async () => {
                return this.projectService.recomputeProjectNodes(targetId, transaction)
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
          }, 1000)
        }),
        catchError(async (error) => {
          console.log('[ROLLBACK TRANSACTION]: Side effect runner')
          await transaction.rollback()
          await this.neogmaService.closeSession(session, 'run-side-effect-interceptor')
          throw error
        })
      )
    }
  }

  return mixin(RunSideEffectInterceptor)
}
