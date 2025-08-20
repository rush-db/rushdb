import { forwardRef, Module } from '@nestjs/common'

import { EntityModule } from '@/core/entity/entity.module'
import { ExportController } from '@/core/entity/import-export/export.controller'
import { ExportService } from '@/core/entity/import-export/export.service'
import { ImportController } from '@/core/entity/import-export/import.controller'
import { ImportService } from '@/core/entity/import-export/import.service'
import { TransactionModule } from '@/core/transactions/transaction.module'
import { ProjectModule } from '@/dashboard/project/project.module'
import { TokenModule } from '@/dashboard/token/token.module'
import { WorkspaceModule } from '@/dashboard/workspace/workspace.module'

@Module({
  imports: [
    // Dashboard modules
    forwardRef(() => TokenModule),
    forwardRef(() => ProjectModule),
    forwardRef(() => WorkspaceModule),

    // Core modules
    forwardRef(() => EntityModule),
    forwardRef(() => TransactionModule)
  ],
  providers: [ImportService, ExportService],
  exports: [ImportService, ExportService],
  controllers: [ImportController, ExportController]
})
export class ImportExportModule {}
