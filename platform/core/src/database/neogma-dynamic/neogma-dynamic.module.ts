import { forwardRef, Module } from '@nestjs/common'

import { NeogmaModule } from '@/database/neogma/neogma.module'
import { NeogmaDynamicService } from '@/database/neogma-dynamic/neogma-dynamic.service'

@Module({
  imports: [forwardRef(() => NeogmaModule)],
  providers: [NeogmaDynamicService],
  exports: [NeogmaDynamicService]
})
export class NeogmaDynamicModule {}
