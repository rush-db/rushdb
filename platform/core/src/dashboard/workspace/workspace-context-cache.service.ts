import { Injectable, Logger } from '@nestjs/common'
import { Transaction } from 'neo4j-driver'
import { WorkspaceContext } from '@/dashboard/workspace/workspace.context'
import { WorkspaceService } from '@/dashboard/workspace/workspace.service'
import { isDevMode } from '@/common/utils/isDevMode'

interface CacheEntry {
  ctx: WorkspaceContext
  lastUsed: number
  timeout: NodeJS.Timeout
}

@Injectable()
export class WorkspaceContextCacheService {
  private cache = new Map<string, CacheEntry>()
  // 5 mins
  private readonly inactivityTimeout = 5 * 60 * 1000

  constructor(private readonly workspaceService: WorkspaceService) {}

  async getWorkspaceContext(workspaceId: string, transaction: Transaction): Promise<WorkspaceContext> {
    const now = Date.now()
    let entry = this.cache.get(workspaceId)

    if (entry) {
      clearTimeout(entry.timeout)
      entry.lastUsed = now
      entry.timeout = this.scheduleCleanup(workspaceId)
      isDevMode(() => Logger.log(`Reusing WorkspaceContext for workspace ${workspaceId}`))

      return entry.ctx
    }

    isDevMode(() => Logger.log(`Building new WorkspaceContext for workspace ${workspaceId}`))
    const ctx = await this.workspaceService.buildWorkspaceContext(workspaceId, transaction)

    const timeout = this.scheduleCleanup(workspaceId)
    entry = { ctx, lastUsed: now, timeout }
    this.cache.set(workspaceId, entry)
    return ctx
  }

  private scheduleCleanup(workspaceId: string): NodeJS.Timeout {
    return setTimeout(() => {
      this.cache.delete(workspaceId)
      isDevMode(() =>
        Logger.log(`WorkspaceContext for workspace ${workspaceId} removed from cache due to inactivity`)
      )
    }, this.inactivityTimeout)
  }
}
