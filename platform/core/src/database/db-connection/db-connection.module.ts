import { Module, forwardRef } from '@nestjs/common'

import { ProjectModule } from '@/dashboard/project/project.module'

import { NeogmaModule } from '../neogma/neogma.module'
import { NeogmaDynamicModule } from '../neogma-dynamic/neogma-dynamic.module'

import { DbConnectionService } from './db-connection.service'

@Module({
  imports: [NeogmaModule, NeogmaDynamicModule, forwardRef(() => ProjectModule)],
  providers: [DbConnectionService],
  exports: [DbConnectionService]
})
export class DbConnectionModule {}
