import { forwardRef, MiddlewareConsumer, Module, NestModule } from '@nestjs/common'

import { EntityModule } from '@/core/entity/entity.module'
import { ImportExportModule } from '@/core/entity/import-export/import-export.module'
import { PropertyModule } from '@/core/property/property.module'
import { QueryModule } from '@/core/query/query.module'
import { TransactionModule } from '@/core/transactions/transaction.module'
import { AuthMiddleware } from '@/dashboard/auth/middlewares/auth.middleware'
import { TokenModule } from '@/dashboard/token/token.module'
import { DbConnectionModule } from '@/database/db-connection/db-connection.module'
import { DbContextMiddleware } from '@/database/middlewares/db-context.middleware'
import { SessionAndTransactionAttachMiddleware } from '@/database/session-and-transaction-attach-middleware.service'

@Module({
  imports: [
    EntityModule,
    PropertyModule,
    ImportExportModule,
    TransactionModule,
    QueryModule,
    forwardRef(() => TokenModule),
    forwardRef(() => DbConnectionModule)
  ],
  exports: [EntityModule, PropertyModule, ImportExportModule, TransactionModule, QueryModule]
})
export class CoreModule implements NestModule {
  configure(consumer: MiddlewareConsumer) {
    consumer.apply(AuthMiddleware, DbContextMiddleware, SessionAndTransactionAttachMiddleware).forRoutes('*')
  }
}
