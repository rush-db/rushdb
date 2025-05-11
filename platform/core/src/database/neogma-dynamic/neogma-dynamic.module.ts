import { forwardRef, Module } from '@nestjs/common'

import { NeogmaModule } from '@/database/neogma/neogma.module'
import { NeogmaDynamicService } from '@/database/neogma-dynamic/neogma-dynamic.service'
import { CompositeNeogmaService } from '@/database/neogma-dynamic/composite-neogma.service'

@Module({
  imports: [forwardRef(() => NeogmaModule)],
  providers: [NeogmaDynamicService, CompositeNeogmaService],
  exports: [NeogmaDynamicService, CompositeNeogmaService]
})
export class NeogmaDynamicModule {}
