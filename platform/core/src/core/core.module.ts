import { forwardRef, MiddlewareConsumer, Module, NestModule } from '@nestjs/common'

import { EntityModule } from '@/core/entity/entity.module'
import { ImportExportModule } from '@/core/entity/import-export/import-export.module'
import { PropertyModule } from '@/core/property/property.module'
import { SearchModule } from '@/core/search/search.module'
import { TransactionModule } from '@/core/transactions/transaction.module'
import { AuthMiddleware } from '@/dashboard/auth/middlewares/auth.middleware'
import { TokenModule } from '@/dashboard/token/token.module'
import { DbConnectionModule } from '@/database/db-connection/db-connection.module'
import { DbContextMiddleware } from '@/database/middlewares/db-context.middleware'
import { SessionAttachMiddleware } from '@/database/session-attach.middleware'

@Module({
  imports: [
    EntityModule,
    PropertyModule,
    ImportExportModule,
    TransactionModule,
    SearchModule,
    forwardRef(() => TokenModule),
    forwardRef(() => DbConnectionModule)
  ],
  exports: [EntityModule, PropertyModule, ImportExportModule, TransactionModule, SearchModule]
})
export class CoreModule implements NestModule {
  configure(consumer: MiddlewareConsumer) {
    consumer.apply(AuthMiddleware, DbContextMiddleware, SessionAttachMiddleware).forRoutes('*')
  }
}
