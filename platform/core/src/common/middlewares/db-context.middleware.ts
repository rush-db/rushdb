import { Injectable, NestMiddleware, Logger } from '@nestjs/common'
import { Request, Response, NextFunction } from 'express'
import { dbContextStorage, DbContext } from '@/database/db-context'
import { DbConnectionService } from '@/database/db-connection/db-connection.service'
import { isDevMode } from '@/common/utils/isDevMode'

@Injectable()
export class DbContextMiddleware implements NestMiddleware {
  constructor(private readonly dbConnectionService: DbConnectionService) {}

  async use(req: Request, res: Response, next: NextFunction) {
    const projectId = req.headers['x-project-id'] as string
    let connection

    if (projectId) {
      try {
        connection = await this.dbConnectionService.getConnection(projectId)
      } catch (e) {
        isDevMode(() => Logger.error(`Error obtaining connection for project ${projectId}`, e))
      }
    }

    if (!connection) {
      connection = await this.dbConnectionService.getConnection('default')
    }

    const context: DbContext = { projectId: projectId || 'default', connection }
    dbContextStorage.run(context, () => next())
  }
}
