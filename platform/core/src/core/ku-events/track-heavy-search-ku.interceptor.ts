import { CallHandler, ExecutionContext, Injectable, mixin, NestInterceptor } from '@nestjs/common'
import { Observable } from 'rxjs'

import { KuOperation } from '@/core/ku-events/ku-events.constants'
import { KuEventsService } from '@/core/ku-events/ku-events.service'
import { isHeavySearch } from '@/core/ku-events/search-complexity'

/**
 * Emits a COMPUTE_OPERATION KU event when the incoming search body is
 * considered "heavy" (aggregations, groupBy, or where nesting > 2 hops).
 *
 * Skipped automatically when:
 *  - RUSHDB_SELF_HOSTED=true   (handled inside KuEventsService.emit)
 *  - project.customDb is set   (external-DB project — user manages their own infra)
 *
 * Usage:
 *   @UseInterceptors(TrackHeavySearchKu())
 *   async mySearchEndpoint(...) { ... }
 */
export const TrackHeavySearchKu = () => {
  @Injectable()
  class TrackHeavySearchKuInterceptor implements NestInterceptor {
    constructor(readonly kuEventsService: KuEventsService) {}

    intercept(context: ExecutionContext, next: CallHandler): Observable<unknown> {
      const request = context.switchToHttp().getRequest()
      // In Fastify the auth middleware writes to request.raw; guards then copy
      // projectId / workspaceId to request directly.  project stays on raw.
      const raw: any = (request as any).raw ?? request

      const projectId: string = request.projectId ?? raw.projectId
      const workspaceId: string = request.workspaceId ?? raw.workspaceId
      const project = raw.project
      const body: Record<string, unknown> = request.body ?? {}

      // Only track shared (non-external) DB instances
      if (!project?.customDb && workspaceId && isHeavySearch(body)) {
        this.kuEventsService.emit(workspaceId, projectId, KuOperation.COMPUTE_OPERATION, {
          type: 'heavy_search'
        })
      }

      return next.handle()
    }
  }

  return mixin(TrackHeavySearchKuInterceptor)
}
