import { Injectable, NestMiddleware, Logger } from '@nestjs/common'
import { Response, NextFunction } from 'express'

import { PlatformRequest } from '@/common/types/request'
import { isDevMode } from '@/common/utils/isDevMode'
import { LOCAL_PROJECT_CONNECTION_LITERAL } from '@/database/db-connection/db-connection.constants'
import { ConnectionResult, DbConnectionService } from '@/database/db-connection/db-connection.service'
import { DbContext, dbContextStorage } from '@/database/db-context'

@Injectable()
export class DbContextMiddleware implements NestMiddleware {
  constructor(private readonly dbConnectionService: DbConnectionService) {}

  async use(request: PlatformRequest, response: Response, next: NextFunction) {
    if (request.method === 'OPTIONS' || request.method === 'HEAD') {
      return next()
    }

    const projectId = request.projectId
    const project = request.project

    const localDbConnection = await this.dbConnectionService.getConnection(LOCAL_PROJECT_CONNECTION_LITERAL)

    let externalConnection: ConnectionResult
    if (projectId && project?.customDb) {
      try {
        externalConnection = await this.dbConnectionService.getConnection(projectId, project)
      } catch (e) {
        isDevMode(() =>
          Logger.error(`[DbContextMiddleware] Error obtaining connection for project ${projectId}`, e)
        )
      }
    }

    const context: DbContext = {
      projectId,
      connection: localDbConnection.connection,
      externalConnection: externalConnection?.connection
    }

    dbContextStorage.run(context, () => next())
  }
}
