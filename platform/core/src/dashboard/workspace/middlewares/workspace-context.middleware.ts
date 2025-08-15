import { Injectable, NestMiddleware, Logger } from '@nestjs/common'
import { Request, Response, NextFunction } from 'express'

import { isDevMode } from '@/common/utils/isDevMode'
import { WorkspaceContextCacheService } from '@/dashboard/workspace/workspace-context-cache.service'
import { WorkspaceContextStorage } from '@/dashboard/workspace/workspace.context'
import { NeogmaService } from '@/database/neogma/neogma.service'

// @Injectable()
// export class WorkspaceContextMiddleware implements NestMiddleware {
//   constructor(
//     private readonly cacheService: WorkspaceContextCacheService,
//     private readonly neogmaService: NeogmaService
//   ) {}
//
//   async use(request: Request, res: Response, next: NextFunction) {
//     const workspaceId = (request as any).workspaceId || request.headers['x-workspace-id']
//
//     if (!workspaceId) {
//       return next()
//     }
//     const session = this.neogmaService.createSession('workspace-context-middleware')
//     const transaction = session.beginTransaction()
//
//     try {
//       const workspaceCtx = await this.cacheService.getWorkspaceContext(workspaceId, transaction)
//
//       WorkspaceContextStorage.run(workspaceCtx, () => {
//         res.on('finish', () => {
//           WorkspaceContextStorage.disable()
//           isDevMode(() => Logger.log(`WorkspaceContextStorage disabled for workspace ${workspaceId}`))
//         })
//         next()
//       })
//     } catch (err) {
//       isDevMode(() => Logger.error(`Error WorkspaceContextStorage for ${workspaceId}`, err))
//       next()
//     } finally {
//       await transaction.commit()
//       await this.neogmaService.closeSession(session, 'workspace-context-middleware')
//     }
//   }
// }
