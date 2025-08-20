import { forwardRef, Module } from '@nestjs/common'

import { EntityQueryService } from '@/core/entity/entity-query.service'
import { EntityController } from '@/core/entity/entity.controller'
import { EntityService } from '@/core/entity/entity.service'
import { LabelsController } from '@/core/labels/controller'
import { PropertyModule } from '@/core/property/property.module'
import { RelationshipsController } from '@/core/relationships/controller'
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
    forwardRef(() => PropertyModule),
    forwardRef(() => TransactionModule)
  ],
  providers: [EntityService, EntityQueryService],
  exports: [EntityService, EntityQueryService],
  controllers: [EntityController, LabelsController, RelationshipsController]
})
export class EntityModule {}
