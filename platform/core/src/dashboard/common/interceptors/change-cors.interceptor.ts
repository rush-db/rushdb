import { ExecutionContext, Injectable, NestInterceptor, CallHandler } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { Observable } from 'rxjs'

import { isDevMode } from '@/common/utils/isDevMode'

@Injectable()
export class ChangeCorsInterceptor implements NestInterceptor {
  constructor(private readonly configService: ConfigService) {}
  intercept(context: ExecutionContext, next: CallHandler): Observable<any> {
    const frontendUrl = this.configService.get<string>('RUSHDB_DASHBOARD_URL')

    const allowedOrigins = isDevMode() ? ['*'] : [frontendUrl]

    const modifiedCorsOptions = {
      allowedHeaders: [
        'Access-Control-Allow-Origin',
        'Origin',
        'X-Requested-With',
        'Accept',
        'Content-Type',
        'Authorization',
        'X-Workspace-Id',
        'X-Project-Id',
        'X-Transaction-Id',
        'Content-Disposition'
      ],
      exposedHeaders: ['Authorization', 'Content-Disposition'],
      credentials: true,
      methods: ['GET', 'PUT', 'OPTIONS', 'POST', 'PATCH', 'DELETE']
    }

    const ctx = context.switchToHttp()
    const request = ctx.getRequest()
    const response = ctx.getResponse()
    const requestOrigin = request.headers.origin

    if (!isDevMode() && allowedOrigins.includes(requestOrigin)) {
      response.header('Access-Control-Allow-Origin', requestOrigin)
    }

    response.header('Access-Control-Allow-Headers', modifiedCorsOptions.allowedHeaders.join(','))
    response.header('Access-Control-Expose-Headers', modifiedCorsOptions.exposedHeaders.join(','))
    response.header('Access-Control-Allow-Credentials', modifiedCorsOptions.credentials.toString())
    response.header('Access-Control-Allow-Methods', modifiedCorsOptions.methods.join(','))

    return next.handle()
  }
}
