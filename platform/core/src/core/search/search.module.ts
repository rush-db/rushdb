import { forwardRef, MiddlewareConsumer, Module, NestModule } from '@nestjs/common'

import { DbContextMiddleware } from '@/common/middlewares/db-context.middleware'
import { EntityModule } from '@/core/entity/entity.module'
import { SearchController } from '@/core/search/search.controller'
import { ProjectModule } from '@/dashboard/project/project.module'
import { TokenModule } from '@/dashboard/token/token.module'
import { WorkspaceModule } from '@/dashboard/workspace/workspace.module'
import { DbConnectionModule } from '@/database/db-connection/db-connection.module'
import { NeogmaDynamicModule } from '@/database/neogma-dynamic/neogma-dynamic.module'

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
  controllers: [SearchController]
})
export class SearchModule implements NestModule {
  configure(consumer: MiddlewareConsumer) {
    consumer.apply(DbContextMiddleware).forRoutes(SearchController)
  }
}
