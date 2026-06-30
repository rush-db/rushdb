import { forwardRef, Module } from '@nestjs/common'

import { ProjectModule } from '@/dashboard/project/project.module'
import { SavedQueryRepository } from '@/dashboard/saved-query/model/saved-query.repository'
import { SavedQueryController } from '@/dashboard/saved-query/saved-query.controller'
import { SavedQueryService } from '@/dashboard/saved-query/saved-query.service'

@Module({
  imports: [forwardRef(() => ProjectModule)],
  providers: [SavedQueryRepository, SavedQueryService],
  exports: [SavedQueryService],
  controllers: [SavedQueryController]
})
export class SavedQueryModule {}
