import { Injectable, Logger } from '@nestjs/common'
import { Neogma } from 'neogma'
import { INeogmaConfig } from '../neogma/neogma-config.interface'
import { createInstance } from '../neogma/neogma.util'
import { isDevMode } from '@/common/utils/isDevMode'
import {
  RUSHDB_KEY_ID,
  RUSHDB_KEY_PROJECT_ID,
  RUSHDB_LABEL_PROPERTY,
  RUSHDB_LABEL_RECORD
} from '@/core/common/constants'
import {
  RUSHDB_LABEL_PROJECT,
  RUSHDB_LABEL_TOKEN,
  RUSHDB_LABEL_USER,
  RUSHDB_LABEL_WORKSPACE
} from '@/dashboard/common/constants'

interface ConnectionEntry {
  connection: Neogma
  lastUsed: number
  timeout: NodeJS.Timeout
}

@Injectable()
export class NeogmaDynamicService {
  private connections = new Map<string, ConnectionEntry>()
  private readonly inactivityTimeout = 5 * 60 * 1000

  async getConnection(projectId: string, config: INeogmaConfig): Promise<Neogma> {
    const now = Date.now()
    let entry = this.connections.get(projectId)

    if (entry) {
      clearTimeout(entry.timeout)
      entry.lastUsed = now
      entry.timeout = this.scheduleCleanup(projectId)
      isDevMode(() => Logger.log(`Reusing dynamic connection for project ${projectId}`))
      return entry.connection
    }

    isDevMode(() => Logger.log(`Creating new dynamic connection for project ${projectId}`))
    const connection = await createInstance(config)
    await this.initializeSchema(connection)
    const timeout = this.scheduleCleanup(projectId)
    entry = { connection, lastUsed: now, timeout }
    this.connections.set(projectId, entry)
    return connection
  }

  private scheduleCleanup(projectId: string): NodeJS.Timeout {
    return setTimeout(() => {
      const entry = this.connections.get(projectId)
      if (entry) {
        if (typeof entry.connection.driver.close === 'function') {
          entry.connection.driver.close()
        }

        this.connections.delete(projectId)
        isDevMode(() => Logger.log(`Dynamic connection for project ${projectId} closed due to inactivity`))
      }
    }, this.inactivityTimeout)
  }

  private async initializeSchema(connection: Neogma): Promise<void> {
    const session = connection.driver.session()
    const transaction = session.beginTransaction()
    try {
      const constraints = [
        `CREATE CONSTRAINT constraint_record_id IF NOT EXISTS FOR (record:${RUSHDB_LABEL_RECORD}) REQUIRE record.${RUSHDB_KEY_ID} IS UNIQUE`,
        `CREATE CONSTRAINT constraint_property_id IF NOT EXISTS FOR (property:${RUSHDB_LABEL_PROPERTY}) REQUIRE property.id IS UNIQUE`
      ]
      const indexes = [
        `CREATE INDEX index_record_id IF NOT EXISTS FOR (n:${RUSHDB_LABEL_RECORD}) ON (n.${RUSHDB_KEY_ID})`,
        `CREATE INDEX index_record_projectid IF NOT EXISTS FOR (n:${RUSHDB_LABEL_RECORD}) ON (n.${RUSHDB_KEY_PROJECT_ID})`,
        `CREATE INDEX index_property_name IF NOT EXISTS FOR (n:${RUSHDB_LABEL_PROPERTY}) ON (n.name)`,
        `CREATE INDEX index_property_mergerer IF NOT EXISTS FOR (n:${RUSHDB_LABEL_PROPERTY}) ON (n.name, n.type, n.projectId, n.metadata)`
      ]

      isDevMode(() => Logger.log('Initializing custom database schema: creating constraints...'))
      for (const constraint of constraints) {
        await transaction.run(constraint)
      }

      isDevMode(() => Logger.log('Initializing custom database schema: creating indexes...'))
      for (const index of indexes) {
        await transaction.run(index)
      }

      isDevMode(() => Logger.log('Initializing RushDB custom db finished...'))
      await transaction.commit()
    } catch (error) {
      isDevMode(() => Logger.error('Error initializing custom DB schema', error))
      await transaction.rollback()
    } finally {
      await session.close()
    }
  }
}
