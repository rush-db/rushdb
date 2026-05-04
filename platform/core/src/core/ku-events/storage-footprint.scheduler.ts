import { Injectable, Logger } from '@nestjs/common'
import { Cron, CronExpression } from '@nestjs/schedule'

import { toBoolean } from '@/common/utils/toBolean'
import { RUSHDB_KEY_PROJECT_ID, RUSHDB_LABEL_RECORD } from '@/core/common/constants'
import { KuOperation } from '@/core/ku-events/ku-events.constants'
import { KuEventsService } from '@/core/ku-events/ku-events.service'
import { ProjectRepository } from '@/dashboard/project/model/project.repository'
import { NeogmaService } from '@/database/neogma/neogma.service'

@Injectable()
export class StorageFootprintScheduler {
  private readonly logger = new Logger(StorageFootprintScheduler.name)

  constructor(
    private readonly neogmaService: NeogmaService,
    private readonly kuEventsService: KuEventsService,
    private readonly projectRepository: ProjectRepository
  ) {}

  /**
   * Runs once per day at midnight.
   *
   * For every workspace that has stored records, emits a STORAGE_FOOTPRINT
   * KU event with the total record count across all projects, plus a
   * per-project breakdown for attribution.
   *
   * Workspace and project metadata are fetched from SQL (source of truth).
   * Record counts are fetched from Neo4j (where records live).
   *
   * Skipped entirely in self-hosted mode (RUSHDB_SELF_HOSTED=true) — the
   * same guard that suppresses all other KU events.
   */
  @Cron(CronExpression.EVERY_DAY_AT_MIDNIGHT)
  async emitDailyStorageFootprint(): Promise<void> {
    if (toBoolean(process.env.RUSHDB_SELF_HOSTED)) {
      return
    }

    try {
      // Fetch all non-deleted, non-customDb projects from SQL (source of truth)
      const projects = await this.projectRepository.findAllWithoutCustomDb()
      if (projects.length === 0) {
        return
      }

      const projectIds = projects.map((p) => p.id)

      // Query Neo4j only for record counts — that's where records live
      const session = this.neogmaService.createSession('storage-footprint-scheduler')
      const transaction = session.beginTransaction({ timeout: 60_000 })

      let recordCountByProject: Map<string, number>

      try {
        const result = await transaction.run(
          `UNWIND $projectIds AS projectId
           MATCH (r:\`${RUSHDB_LABEL_RECORD}\` { \`${RUSHDB_KEY_PROJECT_ID}\`: projectId })
           RETURN projectId, count(r) AS recordCount`,
          { projectIds }
        )

        recordCountByProject = new Map(
          result.records.map((row) => {
            const rawCount = row.get('recordCount')
            const count: number =
              typeof rawCount === 'object' && rawCount !== null && 'toNumber' in rawCount ?
                (rawCount as any).toNumber()
              : Number(rawCount)
            return [row.get('projectId') as string, count]
          })
        )
      } finally {
        await transaction.close()
        await session.close()
      }

      // Aggregate record counts by workspaceId using SQL project→workspace mapping
      const workspaceMap = new Map<string, { projects: Map<string, number>; total: number }>()

      for (const project of projects) {
        const count = recordCountByProject.get(project.id) ?? 0
        if (count === 0) {
          continue
        }

        if (!workspaceMap.has(project.workspaceId)) {
          workspaceMap.set(project.workspaceId, { projects: new Map(), total: 0 })
        }
        const ws = workspaceMap.get(project.workspaceId)!
        ws.projects.set(project.id, count)
        ws.total += count
      }

      // Emit one event per workspace with project breakdown
      let emittedCount = 0
      for (const [workspaceId, { projects: projectMap, total }] of workspaceMap.entries()) {
        const projectBreakdown: Record<string, number> = Object.fromEntries(projectMap)
        const representativeProjectId = projectMap.keys().next().value as string

        this.kuEventsService.emitBulk(
          workspaceId,
          representativeProjectId,
          KuOperation.STORAGE_FOOTPRINT,
          total,
          { source: 'daily-footprint-scheduler', projectBreakdown }
        )
        emittedCount++
      }

      this.logger.log(`[StorageFootprint] Daily storage footprint emitted for ${emittedCount} workspaces`)
    } catch (error) {
      this.logger.error('[StorageFootprint] Failed to emit daily storage footprint', error)
    }
  }
}
