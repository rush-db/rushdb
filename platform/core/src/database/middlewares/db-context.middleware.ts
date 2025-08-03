import { Injectable, NestMiddleware, Logger } from '@nestjs/common'
import { Response, NextFunction } from 'express'

import { PlatformRequest } from '@/common/types/request'
import { isDevMode } from '@/common/utils/isDevMode'
import { ConnectionResult, DbConnectionService } from '@/database/db-connection/db-connection.service'
import { dbContextStorage, DbContext } from '@/database/db-context'

@Injectable()
export class DbContextMiddleware implements NestMiddleware {
  constructor(private readonly dbConnectionService: DbConnectionService) {}

  async use(request: PlatformRequest, response: Response, next: NextFunction) {
    const projectId = request.projectId ?? <string>request.headers['x-project-id']

    let connectionConfig: ConnectionResult

    if (projectId) {
      try {
        connectionConfig = await this.dbConnectionService.getConnection(projectId)
      } catch (e) {
        isDevMode(() => Logger.error(`Error obtaining connection for project ${projectId}`, e))
      }
    }

    if (!connectionConfig) {
      connectionConfig = await this.dbConnectionService.getConnection('default')
    }

    const context: DbContext = {
      projectId: connectionConfig.projectId,
      connection: connectionConfig.connection
    }
    dbContextStorage.run(context, () => next())
  }
}
