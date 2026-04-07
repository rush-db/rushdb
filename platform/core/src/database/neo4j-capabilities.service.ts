import { Injectable, Logger, OnModuleInit } from '@nestjs/common'
import type { Session } from 'neo4j-driver'

import { NeogmaService } from '@/database/neogma/neogma.service'

/** Minimum Neo4j version required to run RushDB. */
const MIN_YEAR = 2026
const MIN_MONTH = 1
const MIN_PATCH = 4

/**
 * Validates the connected Neo4j version at startup (shared instance).
 * RushDB requires Neo4j ≥ 2026.01.4 for native conditional subquery support.
 *
 * Also exposes a static helper reusable by NeogmaDynamicService for BYOV validation.
 */
@Injectable()
export class Neo4jCapabilitiesService implements OnModuleInit {
  private readonly logger = new Logger(Neo4jCapabilitiesService.name)

  constructor(private readonly neogmaService: NeogmaService) {}

  async onModuleInit() {
    const session = this.neogmaService.createSession('capabilities-check')
    try {
      const version = await Neo4jCapabilitiesService.getVersionFromSession(session)
      Neo4jCapabilitiesService.assertVersionSupported(version)
      this.logger.log(`Neo4j version ${version} — OK`)
    } catch (error) {
      if ((error as Error)?.message?.includes('not supported')) {
        throw error
      }
      this.logger.warn(
        `Could not determine Neo4j version — proceeding with caution. Error: ${(error as Error)?.message}`
      )
    } finally {
      await this.neogmaService.closeSession(session)
    }
  }

  /**
   * Reads the Neo4j version from an already-open session.
   * Works with both shared and external (BYOV) connections.
   */
  static async getVersionFromSession(session: Session): Promise<string> {
    const result = await session.run('CALL dbms.components() YIELD versions RETURN versions[0] AS version')
    return (result.records[0]?.get('version') as string) ?? '0.0.0'
  }

  /**
   * Throws if the version string does not satisfy the minimum requirement.
   * Silently passes for non-calendar versions (e.g. Aura "5.x") since we
   * cannot reliably determine compatibility.
   */
  static assertVersionSupported(version: string): void {
    const [year, month, patch] = version.split('.').map(Number)

    // Non-calendar format (e.g. Aura "5.26.0") — cannot validate
    if (year < 2000) return

    const supported =
      year > MIN_YEAR ||
      (year === MIN_YEAR && month > MIN_MONTH) ||
      (year === MIN_YEAR && month === MIN_MONTH && patch >= MIN_PATCH)
    if (!supported) {
      throw new Error(
        `Neo4j version ${version} is not supported. RushDB requires Neo4j ≥ ${MIN_YEAR}.${String(MIN_MONTH).padStart(2, '0')}.${MIN_PATCH}.`
      )
    }
  }
}
