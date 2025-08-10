import { Injectable, Logger } from '@nestjs/common'
import { Session } from 'neo4j-driver'

import { CompositeNeogmaService } from '@/database/neogma-dynamic/composite-neogma.service'

import { NeogmaService } from './neogma/neogma.service'

/**
 * Centralized session manager for Neo4j sessions.
 * Handles shared (internal) and per-project (external) session reuse.
 */
@Injectable()
export class SessionManagerService {
  private sharedSession: Session | null = null
  private externalSessions: Map<string, Session> = new Map()

  constructor(
    private readonly neogmaService: NeogmaService
    // private readonly compositeNeogmaService: CompositeNeogmaService
  ) {}

  /**
   * Get or create a session for a given projectId.
   * - For shared DB, reuse singleton session.
   * - For external DBs, reuse per-project session.
   */
  getOrCreateSession(projectId: string): Session {
    if (projectId === 'default' || !projectId) {
      if (!this.sharedSession) {
        this.sharedSession = this.neogmaService.createSession()
        Logger.debug('[SessionManager] Created shared session')
      }
      return this.sharedSession
    } else {
      if (!this.externalSessions.has(projectId)) {
        const session = this.neogmaService.createSession()
        this.externalSessions.set(projectId, session)
        Logger.debug(`[SessionManager] Created session for project ${projectId}`)
      }
      return this.externalSessions.get(projectId)
    }
  }

  /**
   * Optionally, implement session cleanup/eviction logic here.
   */
  // cleanupSessions() { ... }
}
