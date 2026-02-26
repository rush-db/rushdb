import { forwardRef, Module } from '@nestjs/common'

import { BillingClientModule } from '@/core/billing-client/billing-client.module'
import { EntityModule } from '@/core/entity/entity.module'
import { QueryController } from '@/core/query/query.controller'
import { WorkspaceModule } from '@/dashboard/workspace/workspace.module'

@Module({
  imports: [BillingClientModule, forwardRef(() => WorkspaceModule), forwardRef(() => EntityModule)],
  controllers: [QueryController]
})
export class QueryModule {}
