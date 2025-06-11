import { Module } from '@nestjs/common'

import { EntityModule } from '@/core/entity/entity.module'
import { ImportExportModule } from '@/core/entity/import-export/import-export.module'
import { PropertyModule } from '@/core/property/property.module'
import { SearchModule } from '@/core/search/search.module'
import { TransactionModule } from '@/core/transactions/transaction.module'

@Module({
  imports: [EntityModule, PropertyModule, ImportExportModule, TransactionModule, SearchModule],
  exports: [EntityModule, PropertyModule, ImportExportModule, TransactionModule, SearchModule]
})
export class CoreModule {}
