import { Injectable, NestMiddleware, Logger } from '@nestjs/common'
import { Request, Response, NextFunction } from 'express'

import { isDevMode } from '@/common/utils/isDevMode'
import { ConnectionResult, DbConnectionService } from '@/database/db-connection/db-connection.service'
import { dbContextStorage, DbContext } from '@/database/db-context'

@Injectable()
export class DbContextMiddleware implements NestMiddleware {
  constructor(private readonly dbConnectionService: DbConnectionService) {}

  async use(req: Request, res: Response, next: NextFunction) {
    const projectId = req.headers['x-project-id'] as string
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
