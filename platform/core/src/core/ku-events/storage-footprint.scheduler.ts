import { Injectable, Logger } from '@nestjs/common'
import { Cron, CronExpression } from '@nestjs/schedule'

import { RUSHDB_KEY_PROJECT_ID, RUSHDB_LABEL_RECORD } from '@/core/common/constants'
import {
  RUSHDB_LABEL_WORKSPACE,
  RUSHDB_LABEL_PROJECT,
  RUSHDB_RELATION_CONTAINS
} from '@/dashboard/common/constants'
import { KuOperation } from '@/core/ku-events/ku-events.constants'
import { KuEventsService } from '@/core/ku-events/ku-events.service'
import { toBoolean } from '@/common/utils/toBolean'
import { NeogmaService } from '@/database/neogma/neogma.service'

@Injectable()
export class StorageFootprintScheduler {
  private readonly logger = new Logger(StorageFootprintScheduler.name)

  constructor(
    private readonly neogmaService: NeogmaService,
    private readonly kuEventsService: KuEventsService
  ) {}

  /**
   * Runs once per day at midnight.
   *
   * For every workspace that has stored records, emits a STORAGE_FOOTPRINT
   * KU event with the total record count across all projects, plus a
   * per-project breakdown for attribution.
   *
   * The billing service uses this to apply a daily prorated storage charge
   * at the workspace level, ensuring that large idle datasets (e.g. 5 TB
   * ingested once) continue to generate revenue that covers ongoing
   * infrastructure costs.
   *
   * Skipped entirely in self-hosted mode (RUSHDB_SELF_HOSTED=true) — the
   * same guard that suppresses all other KU events.
   */
  @Cron(CronExpression.EVERY_DAY_AT_MIDNIGHT)
  async emitDailyStorageFootprint(): Promise<void> {
    if (toBoolean(process.env.RUSHDB_SELF_HOSTED)) {
      return
    }

    const session = this.neogmaService.createSession('storage-footprint-scheduler')
    const transaction = session.beginTransaction({ timeout: 60_000 })

    try {
      // Query workspace -> project -> record hierarchy
      // Group by workspace and project to get per-project counts
      // Exclude custom DB projects (users manage their own storage)
      const result = await transaction.run(
        `MATCH (workspace:\`${RUSHDB_LABEL_WORKSPACE}\`)-[:\`${RUSHDB_RELATION_CONTAINS}\`]->(project:\`${RUSHDB_LABEL_PROJECT}\`)
         WHERE project.customDb IS NULL
         WITH workspace, project
         MATCH (record:\`${RUSHDB_LABEL_RECORD}\` { \`${RUSHDB_KEY_PROJECT_ID}\`: project.id })
         RETURN workspace.id AS workspaceId, project.id AS projectId, count(record) AS recordCount
         ORDER BY workspace.id, project.id`
      )

      await transaction.close()
      await session.close()

      // Group results by workspace
      const workspaceMap = new Map<string, { projects: Map<string, number>; total: number }>()

      for (const row of result.records) {
        const workspaceId = row.get('workspaceId') as string | null
        const projectId = row.get('projectId') as string | null
        const rawCount = row.get('recordCount')
        const recordCount: number =
          typeof rawCount === 'object' && rawCount !== null && 'toNumber' in rawCount ?
            (rawCount as any).toNumber()
          : Number(rawCount)

        if (workspaceId && projectId && recordCount > 0) {
          if (!workspaceMap.has(workspaceId)) {
            workspaceMap.set(workspaceId, { projects: new Map(), total: 0 })
          }
          const workspace = workspaceMap.get(workspaceId)!
          workspace.projects.set(projectId, recordCount)
          workspace.total += recordCount
        }
      }

      // Emit one event per workspace with project breakdown
      let emittedCount = 0
      for (const [workspaceId, { projects, total }] of workspaceMap.entries()) {
        // Convert project map to plain object for metadata
        const projectBreakdown: Record<string, number> = {}
        for (const [projectId, count] of projects.entries()) {
          projectBreakdown[projectId] = count
        }

        // Emit with total count and project breakdown in metadata
        // Note: We use emitBulk with total count, and include project details in metadata
        this.kuEventsService.emitBulk(
          workspaceId,
          Object.keys(projectBreakdown)[0], // Use first project as representative projectId
          KuOperation.STORAGE_FOOTPRINT,
          total,
          {
            source: 'daily-footprint-scheduler',
            projectBreakdown
          }
        )
        emittedCount++
      }

      this.logger.log(
        `[StorageFootprint] Daily storage footprint emitted for ${emittedCount} workspaces (${workspaceMap.size} total)`
      )
    } catch (error) {
      this.logger.error('[StorageFootprint] Failed to emit daily storage footprint', error)
      try {
        await transaction.rollback()
      } catch (_) {
        // ignore rollback errors on a read-only query
      }
      await session.close()
    }
  }
}
