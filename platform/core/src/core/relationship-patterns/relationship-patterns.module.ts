import { forwardRef, Module } from '@nestjs/common'

import { AiModule } from '@/core/ai/ai.module'
import { EntityModule } from '@/core/entity/entity.module'
import { ProjectRepository } from '@/dashboard/project/model/project.repository'

import { RelationshipPatternsController } from './relationship-patterns.controller'
import { RelationshipPatternsRepository } from './relationship-patterns.repository'
import { RelationshipPatternsScheduler } from './relationship-patterns.scheduler'
import { RelationshipPatternsService } from './relationship-patterns.service'

@Module({
  imports: [forwardRef(() => AiModule), forwardRef(() => EntityModule)],
  providers: [
    RelationshipPatternsRepository,
    RelationshipPatternsService,
    RelationshipPatternsScheduler,
    ProjectRepository
  ],
  exports: [RelationshipPatternsService],
  controllers: [RelationshipPatternsController]
})
export class RelationshipPatternsModule {}
