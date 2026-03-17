import { forwardRef, Module } from '@nestjs/common'

import { BillingClientModule } from '@/core/billing-client/billing-client.module'
import { EntityModule } from '@/core/entity/entity.module'
import { PropertyQueryService } from '@/core/property/property-query.service'
import { PropertyController } from '@/core/property/property.controller'
import { PropertyService } from '@/core/property/property.service'
import { ProjectModule } from '@/dashboard/project/project.module'
import { TokenModule } from '@/dashboard/token/token.module'
import { WorkspaceModule } from '@/dashboard/workspace/workspace.module'

@Module({
  imports: [
    BillingClientModule,
    forwardRef(() => EntityModule),
    forwardRef(() => TokenModule),
    forwardRef(() => ProjectModule),
    forwardRef(() => WorkspaceModule)
  ],
  providers: [PropertyService, PropertyQueryService],
  exports: [PropertyService, PropertyQueryService],
  controllers: [PropertyController]
})
export class PropertyModule {}
