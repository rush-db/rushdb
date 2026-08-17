import { forwardRef, Module } from '@nestjs/common'

import { EntityModule } from '@/core/entity/entity.module'
import { ConnectorSecretService } from '@/dashboard/connector/connector-secret.service'
import { ConnectorController } from '@/dashboard/connector/connector.controller'
import { ConnectorService } from '@/dashboard/connector/connector.service'
import { ConnectorRepository } from '@/dashboard/connector/model/connector.repository'
import { ProjectModule } from '@/dashboard/project/project.module'
import { SynxConnectorRegistry } from '@/dashboard/synx/synx.connectors'
import { TokenModule } from '@/dashboard/token/token.module'
import { WorkspaceModule } from '@/dashboard/workspace/workspace.module'

@Module({
  imports: [
    forwardRef(() => ProjectModule),
    forwardRef(() => TokenModule),
    forwardRef(() => EntityModule),
    forwardRef(() => WorkspaceModule)
  ],
  providers: [ConnectorRepository, ConnectorSecretService, ConnectorService, SynxConnectorRegistry],
  exports: [ConnectorRepository, ConnectorService, SynxConnectorRegistry],
  controllers: [ConnectorController]
})
export class ConnectorModule {}
