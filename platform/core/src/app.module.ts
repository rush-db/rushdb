import { Global, Module } from '@nestjs/common'
import { ConfigModule } from '@nestjs/config'
import { APP_INTERCEPTOR } from '@nestjs/core'
import { ServeStaticModule } from '@nestjs/serve-static'
import { ThrottlerModule } from '@nestjs/throttler'
import { ConsoleModule } from 'nestjs-console'

import { AppSettingsController } from '@/app-settings.controller'
import { AppController } from '@/app.controller'
import { AppService } from '@/app.service'
import { BackupModule } from '@/backup/backup.module'
import { CliService } from '@/cli/cli.service'
import { ExcludeNullInterceptor } from '@/common/interceptors/exclude-null-response.interceptor'
import { toBoolean } from '@/common/utils/toBolean'
import { CoreModule } from '@/core/core.module'
import { KuEventsModule } from '@/core/ku-events/ku-events.module'
import { DashboardModule } from '@/dashboard/dashboard.module'
import { ThrottleService } from '@/dashboard/throttle/throttle.service'
import { DatabaseModule } from '@/database/database.module'
import { RequestCleanupInterceptor } from '@/database/interceptors/request-cleanup.interceptor'
import { HealthController } from '@/health.controller'

import { join } from 'path'

@Global()
@Module({
  imports: [
    ThrottlerModule.forRootAsync({
      useClass: ThrottleService
    }),
    ConfigModule.forRoot({ isGlobal: true }),
    DatabaseModule,
    KuEventsModule,
    CoreModule,
    DashboardModule,
    ...(toBoolean(process.env.RUSHDB_SERVE_STATIC) ?
      [
        ServeStaticModule.forRoot({
          rootPath: join(__dirname, '..', 'public'),
          renderPath: '/*',
          // Fastify loader only: the catch-all renderFn serves the SPA index.html
          // ONLY when serveStaticOptions.fallthrough is true; otherwise it returns
          // 404. Without this, hard-reloading any non-root client route (e.g.
          // /projects/abc) 404s because no matching static file exists and the
          // fallback refuses to serve index.html. (The Express loader didn't gate
          // on this; the Nest 11 / @nestjs/serve-static v5 Fastify loader does.)
          serveStaticOptions: { fallthrough: true },
          // NOTE: `exclude` is honored by the Express loader only — the Fastify
          // loader ignores it. API routes are real registered routes under the
          // `api/v1` global prefix, so find-my-way matches them before the `/*`
          // wildcard; the SPA fallback only catches unrouted (client) paths.
          exclude: ['/api*', '/health']
        })
      ]
    : []),
    ConsoleModule,
    BackupModule
  ],
  providers: [
    AppService,
    {
      provide: APP_INTERCEPTOR,
      useClass: ExcludeNullInterceptor
    },
    {
      provide: APP_INTERCEPTOR,
      useClass: RequestCleanupInterceptor
    },
    CliService
  ],
  // AppController serves GET '/' as a liveness string. When RUSHDB_SERVE_STATIC
  // is on, the dashboard SPA (via @fastify/static) owns '/', and registering both
  // throws "Method 'HEAD' already declared for route '/'" under Fastify 5. Drop the
  // root controller in static mode; /health (HealthController) covers liveness either way.
  controllers: [
    ...(toBoolean(process.env.RUSHDB_SERVE_STATIC) ? [] : [AppController]),
    HealthController,
    AppSettingsController
  ]
})
export class AppModule {}
