import { forwardRef, Module } from '@nestjs/common'

import { AiQueryService } from '@/core/ai/ai-query.service'
import { AiController } from '@/core/ai/ai.controller'
import { AiService } from '@/core/ai/ai.service'
import { EmbeddingBackfillScheduler } from '@/core/ai/embedding-backfill.scheduler'
import { EmbeddingIndexDdlService } from '@/core/ai/embedding-index-ddl.service'
import { EmbeddingIndexRepository } from '@/core/ai/embedding-index.repository'
import { EmbeddingProviderService } from '@/core/ai/embedding-provider.service'
import { BillingPolicyModule } from '@/core/billing-policy/billing-policy.module'
import { EntityModule } from '@/core/entity/entity.module'
import { ProjectModule } from '@/dashboard/project/project.module'
import { TokenModule } from '@/dashboard/token/token.module'
import { WorkspaceModule } from '@/dashboard/workspace/workspace.module'

@Module({
  imports: [
    BillingPolicyModule,
    forwardRef(() => EntityModule),
    forwardRef(() => TokenModule),
    forwardRef(() => ProjectModule),
    forwardRef(() => WorkspaceModule)
  ],
  providers: [
    AiService,
    AiQueryService,
    EmbeddingIndexDdlService,
    EmbeddingIndexRepository,
    EmbeddingProviderService,
    EmbeddingBackfillScheduler
  ],
  exports: [AiService, EmbeddingProviderService],
  controllers: [AiController]
})
export class AiModule {}
