import { Module } from '@nestjs/common'

import { EntityModule } from '@/core/entity/entity.module'
import { ImportExportModule } from '@/core/entity/import-export/import-export.module'
import { PropertyModule } from '@/core/property/property.module'
import { TransactionModule } from '@/core/transactions/transaction.module'

@Module({
  imports: [EntityModule, PropertyModule, ImportExportModule, TransactionModule],
  exports: [EntityModule, PropertyModule, ImportExportModule, TransactionModule]
})
export class CoreModule {}
