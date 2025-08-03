import { forwardRef, MiddlewareConsumer, Module } from '@nestjs/common'

import { EntityModule } from '@/core/entity/entity.module'
import { ImportExportModule } from '@/core/entity/import-export/import-export.module'
import { PropertyModule } from '@/core/property/property.module'
import { SearchModule } from '@/core/search/search.module'
import { TransactionModule } from '@/core/transactions/transaction.module'
import { AuthMiddleware } from '@/dashboard/auth/middlewares/auth.middleware'
import { TokenModule } from '@/dashboard/token/token.module'

@Module({
  imports: [
    EntityModule,
    PropertyModule,
    ImportExportModule,
    TransactionModule,
    SearchModule,
    forwardRef(() => TokenModule)
  ],
  exports: [EntityModule, PropertyModule, ImportExportModule, TransactionModule, SearchModule]
})
export class CoreModule {
  configure(consumer: MiddlewareConsumer) {
    consumer.apply(AuthMiddleware).forRoutes('*')
  }
}
