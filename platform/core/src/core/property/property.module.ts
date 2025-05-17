import { forwardRef, MiddlewareConsumer, Module, NestModule } from '@nestjs/common'

import { EntityModule } from '@/core/entity/entity.module'
import { PropertyQueryService } from '@/core/property/property-query.service'
import { PropertyController } from '@/core/property/property.controller'
import { PropertyService } from '@/core/property/property.service'
import { ProjectModule } from '@/dashboard/project/project.module'
import { TokenModule } from '@/dashboard/token/token.module'

import { PropertyRepository } from './model/property.repository'
import { NeogmaDynamicModule } from '@/database/neogma-dynamic/neogma-dynamic.module'
import { DbConnectionModule } from '@/database/db-connection/db-connection.module'
import { DbContextMiddleware } from '@/common/middlewares/db-context.middleware'
import { WorkspaceModule } from '@/dashboard/workspace/workspace.module'

@Module({
  imports: [
    forwardRef(() => EntityModule),
    forwardRef(() => TokenModule),
    forwardRef(() => ProjectModule),
    forwardRef(() => WorkspaceModule),

    //db modules
    forwardRef(() => NeogmaDynamicModule),
    forwardRef(() => DbConnectionModule)
  ],
  providers: [PropertyRepository, PropertyService, PropertyQueryService],
  exports: [PropertyRepository, PropertyService, PropertyQueryService],
  controllers: [PropertyController]
})
export class PropertyModule implements NestModule {
  configure(consumer: MiddlewareConsumer) {
    consumer.apply(DbContextMiddleware).forRoutes(PropertyController)
  }
}
