import { Injectable, NestMiddleware, Logger } from '@nestjs/common'
import { Response, NextFunction } from 'express'

import { PlatformRequest } from '@/common/types/request'
import { isDevMode } from '@/common/utils/isDevMode'
import { LOCAL_PROJECT_CONNECTION_LITERAL } from '@/database/db-connection/db-connection.constants'
import { ConnectionResult, DbConnectionService } from '@/database/db-connection/db-connection.service'
// import { dbContextStorage, DbContext } from '@/database/db-context'

@Injectable()
export class DbContextMiddleware implements NestMiddleware {
  constructor(private readonly dbConnectionService: DbConnectionService) {}

  async use(request: PlatformRequest, response: Response, next: NextFunction) {
    const projectId = request.projectId
    const project = request.project

    request.localDbConnection = await this.dbConnectionService.getConnection(LOCAL_PROJECT_CONNECTION_LITERAL)

    // let connectionConfig: ConnectionResult

    if (projectId && project?.customDb) {
      try {
        request.externalDbConnection = await this.dbConnectionService.getConnection(projectId, project)
      } catch (e) {
        isDevMode(() => Logger.error(`Error obtaining connection for project ${projectId}`, e))
      }
    }

    // const context: DbContext = {
    //   projectId: connectionConfig.projectId,
    //   connection: connectionConfig.connection
    // }

    // request.externalDbConnection = connectionConfig.connection
    // request.localDbConnection = connectionConfig.connection

    // dbContextStorage.run(context, () => next())
    next()
  }
}
