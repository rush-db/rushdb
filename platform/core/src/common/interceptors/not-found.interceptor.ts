import { CallHandler, ExecutionContext, Injectable, NestInterceptor, NotFoundException } from '@nestjs/common'
import { Observable } from 'rxjs'
import { tap } from 'rxjs/operators'

// Worker-polling control-plane routes return `null` to signal "no work" —
// that is an expected empty state, not a missing resource, so the interceptor
// must not rewrite it into a 404 (which would spam the error log each poll).
// The route path carries the global `api/v1` prefix, so match on the suffix.
const NULLABLE_POLL_ROUTE_SUFFIXES = ['/connectors/_internal/claim', '/connectors/_internal/commands/claim']

@Injectable()
export class NotFoundInterceptor implements NestInterceptor {
  intercept(context: ExecutionContext, next: CallHandler): Observable<any> {
    const request = context.switchToHttp().getRequest<{ route?: { path?: string }; url?: string }>()
    const path = request?.route?.path ?? request?.url ?? ''
    const isPollRoute = NULLABLE_POLL_ROUTE_SUFFIXES.some((suffix) => path.endsWith(suffix))
    if (isPollRoute) {
      return next.handle()
    }
    return next.handle().pipe(
      tap((data) => {
        if (data === undefined || data === null) {
          throw new NotFoundException()
        }
      })
    )
  }
}
