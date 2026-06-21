import { forwardRef, Module } from '@nestjs/common'

import { ConnectorSecretService } from '@/dashboard/connector/connector-secret.service'
import { ConnectorController } from '@/dashboard/connector/connector.controller'
import { ConnectorService } from '@/dashboard/connector/connector.service'
import { ConnectorRepository } from '@/dashboard/connector/model/connector.repository'
import { ProjectModule } from '@/dashboard/project/project.module'
import { TokenModule } from '@/dashboard/token/token.module'

@Module({
  imports: [forwardRef(() => ProjectModule), forwardRef(() => TokenModule)],
  providers: [ConnectorRepository, ConnectorSecretService, ConnectorService],
  exports: [ConnectorRepository, ConnectorService],
  controllers: [ConnectorController]
})
export class ConnectorModule {}
