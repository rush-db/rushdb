import { forwardRef, MiddlewareConsumer, Module, NestModule } from '@nestjs/common'

import { DbContextMiddleware } from '@/common/middlewares/db-context.middleware'
import { EntityQueryService } from '@/core/entity/entity-query.service'
import { EntityController } from '@/core/entity/entity.controller'
import { EntityService } from '@/core/entity/entity.service'
import { EntityRepository } from '@/core/entity/model/entity.repository'
import { LabelsController } from '@/core/labels/controller'
import { PropertyModule } from '@/core/property/property.module'
import { RelationshipsController } from '@/core/relationships/controller'
import { TransactionModule } from '@/core/transactions/transaction.module'
import { ProjectModule } from '@/dashboard/project/project.module'
import { TokenModule } from '@/dashboard/token/token.module'
import { WorkspaceModule } from '@/dashboard/workspace/workspace.module'
import { DbConnectionModule } from '@/database/db-connection/db-connection.module'
import { NeogmaDynamicModule } from '@/database/neogma-dynamic/neogma-dynamic.module'

@Module({
  imports: [
    // Dashboard modules
    forwardRef(() => TokenModule),
    forwardRef(() => ProjectModule),
    forwardRef(() => WorkspaceModule),

    // Core modules
    forwardRef(() => PropertyModule),
    forwardRef(() => TransactionModule),

    //db modules
    forwardRef(() => NeogmaDynamicModule),
    forwardRef(() => DbConnectionModule)
  ],
  providers: [EntityRepository, EntityService, EntityQueryService],
  exports: [EntityRepository, EntityService, EntityQueryService],
  controllers: [EntityController, LabelsController, RelationshipsController]
})
export class EntityModule implements NestModule {
  configure(consumer: MiddlewareConsumer) {
    consumer.apply(DbContextMiddleware).forRoutes(EntityController, LabelsController)
  }
}
