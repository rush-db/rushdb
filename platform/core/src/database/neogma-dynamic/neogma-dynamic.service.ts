import { Injectable, Logger } from '@nestjs/common'
import { Neogma } from 'neogma'
import { INeogmaConfig } from '../neogma/neogma-config.interface'
import { createInstance } from '../neogma/neogma.util'
import { isDevMode } from '@/common/utils/isDevMode'

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
        `CREATE CONSTRAINT constraint_user_login IF NOT EXISTS FOR (user:User) REQUIRE user.login IS UNIQUE`,
        `CREATE CONSTRAINT constraint_record_id IF NOT EXISTS FOR (record:Record) REQUIRE record.id IS UNIQUE`,
        `CREATE CONSTRAINT constraint_property_id IF NOT EXISTS FOR (property:Property) REQUIRE property.id IS UNIQUE`
      ]
      const indexes = [
        `CREATE INDEX index_record_id IF NOT EXISTS FOR (n:Record) ON (n.id)`,
        `CREATE INDEX index_record_projectid IF NOT EXISTS FOR (n:Record) ON (n.projectId)`,
        `CREATE INDEX index_property_name IF NOT EXISTS FOR (n:Property) ON (n.name)`,
        `CREATE INDEX index_property_mergerer IF NOT EXISTS FOR (n:Property) ON (n.name, n.type, n.projectId, n.metadata)`
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
