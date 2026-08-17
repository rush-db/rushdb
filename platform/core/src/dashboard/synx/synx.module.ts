import { Module } from '@nestjs/common'

import { CoreModule } from '@/core/core.module'
import { ConnectorModule } from '@/dashboard/connector/connector.module'
import { ProjectModule } from '@/dashboard/project/project.module'

import { SynxConnectorRegistry } from './synx.connectors'
import { SynxController } from './synx.controller'
import { SynxService } from './synx.service'

@Module({
  imports: [CoreModule, ConnectorModule, ProjectModule],
  providers: [SynxService, SynxConnectorRegistry],
  controllers: [SynxController]
})
export class SynxModule {}
