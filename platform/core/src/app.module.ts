import { Global, Module } from '@nestjs/common'
import { ConfigModule } from '@nestjs/config'
import { APP_INTERCEPTOR } from '@nestjs/core'
import { ServeStaticModule } from '@nestjs/serve-static'
import { ThrottlerModule } from '@nestjs/throttler'

import { AppController } from '@/app.controller'
import { AppService } from '@/app.service'
import { ExcludeNullInterceptor } from '@/common/interceptors/exclude-null-response.interceptor'
import { CoreModule } from '@/core/core.module'
import { DashboardModule } from '@/dashboard/dashboard.module'
import { ThrottleService } from '@/dashboard/throttle/throttle.service'
import { DatabaseModule } from '@/database/database.module'

import { join } from 'path'

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
    ServeStaticModule.forRoot({
      rootPath: join(__dirname, '..', 'public'),
      renderPath: '/*',
      exclude: ['/api*']
    })
  ],
  providers: [
    AppService,
    {
      provide: APP_INTERCEPTOR,
      useClass: ExcludeNullInterceptor
    }
  ],
  controllers: [AppController]
})
export class AppModule {}
