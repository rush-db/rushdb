import { Module, forwardRef } from '@nestjs/common'
import { DbConnectionService } from './db-connection.service'
import { NeogmaDynamicModule } from '../neogma-dynamic/neogma-dynamic.module'
import { NeogmaModule } from '../neogma/neogma.module'
import { ProjectModule } from '@/dashboard/project/project.module'

@Module({
  imports: [NeogmaModule, NeogmaDynamicModule, forwardRef(() => ProjectModule)],
  providers: [DbConnectionService],
  exports: [DbConnectionService]
})
export class DbConnectionModule {}
