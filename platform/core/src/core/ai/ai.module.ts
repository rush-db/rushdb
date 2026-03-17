import { forwardRef, Module } from '@nestjs/common'

import { AiController } from '@/core/ai/ai.controller'
import { AiQueryService } from '@/core/ai/ai-query.service'
import { AiService } from '@/core/ai/ai.service'
import { EntityModule } from '@/core/entity/entity.module'
import { ProjectModule } from '@/dashboard/project/project.module'
import { TokenModule } from '@/dashboard/token/token.module'
import { WorkspaceModule } from '@/dashboard/workspace/workspace.module'

@Module({
  imports: [
    forwardRef(() => EntityModule),
    forwardRef(() => TokenModule),
    forwardRef(() => ProjectModule),
    forwardRef(() => WorkspaceModule)
  ],
  providers: [AiService, AiQueryService],
  exports: [AiService],
  controllers: [AiController]
})
export class AiModule {}
