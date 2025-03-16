import { Global, Module, DynamicModule, Provider } from '@nestjs/common'
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
import { DashboardModule } from '@/dashboard/dashboard.module'
import { ThrottleService } from '@/dashboard/throttle/throttle.service'
import { DatabaseModule } from '@/database/database.module'

import { join } from 'path'

let internalModule: DynamicModule | null = null
let internalServiceProvider: Provider | null = null

if (process.env.RUSHDB_SELF_HOSTED === 'false') {
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const { InternalModule, InternalService } = require('@rush-db/core-internal-module')

    internalModule = InternalModule
    internalServiceProvider = {
      provide: 'INTERNAL_SERVICE',
      useClass: InternalService
    }
  } catch (error) {
    console.warn('Warning: @rush-db/core-internal-module is not available. Running without it.')
  }
}

@Global()
@Module({
  imports: [
    ThrottlerModule.forRootAsync({
      useClass: ThrottleService
    }),
    ConfigModule.forRoot({ isGlobal: true }),
    DatabaseModule,
    CoreModule,
    DashboardModule,
    ...(toBoolean(process.env.RUSHDB_SERVE_STATIC) ?
      [
        ServeStaticModule.forRoot({
          rootPath: join(__dirname, '..', 'public'),
          renderPath: '/*',
          exclude: ['/api*']
        })
      ]
    : []),
    ...(internalModule ? [internalModule] : []),
    ConsoleModule,
    BackupModule
  ],
  providers: [
    AppService,
    {
      provide: APP_INTERCEPTOR,
      useClass: ExcludeNullInterceptor
    },
    CliService,
    ...(internalServiceProvider ? [internalServiceProvider] : [])
  ],
  controllers: [
    ...(!toBoolean(process.env.RUSHDB_SERVE_STATIC) ? [AppController] : []),
    AppSettingsController
  ]
})
export class AppModule {}
