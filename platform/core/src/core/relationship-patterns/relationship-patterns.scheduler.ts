import { Injectable, Logger } from '@nestjs/common'
import { Cron } from '@nestjs/schedule'

import { RelationshipPatternsService } from './relationship-patterns.service'

@Injectable()
export class RelationshipPatternsScheduler {
  private readonly logger = new Logger(RelationshipPatternsScheduler.name)
  private running = false

  constructor(private readonly relationshipPatternsService: RelationshipPatternsService) {}

  @Cron('* * * * *')
  async processDueAnalysis(): Promise<void> {
    if (this.running) {
      return
    }
    this.running = true
    try {
      await this.relationshipPatternsService.processDueAnalysis()
    } catch (error) {
      this.logger.error('[RelationshipPatternsScheduler] failed', error)
    } finally {
      this.running = false
    }
  }
}
