import { Global, Module, OnModuleInit, Logger } from '@nestjs/common'
import { ConfigModule, ConfigService } from '@nestjs/config'

import { fetchRetry } from '@/common/utils/fetchRetry'
import { isDevMode } from '@/common/utils/isDevMode'
import {
  RUSHDB_KEY_ID,
  RUSHDB_KEY_PROJECT_ID,
  RUSHDB_LABEL_PROPERTY,
  RUSHDB_LABEL_RECORD
} from '@/core/common/constants'
import { Neo4jCapabilitiesService } from '@/database/neo4j-capabilities.service'
import { INeogmaConfig } from '@/database/neogma/neogma-config.interface'
import { NeogmaModule } from '@/database/neogma/neogma.module'
import { NeogmaService } from '@/database/neogma/neogma.service'
import { SqlModule } from '@/database/sql/sql.module'
import { DEFAULT_TRANSACTION_TIMEOUT_MS } from '@/database/transaction.constants'

@Global()
@Module({
  imports: [
    NeogmaModule.forRootAsync({
      imports: [ConfigModule],
      inject: [ConfigService],
      useFactory: (configService: ConfigService): INeogmaConfig => ({
        url: configService.get('NEO4J_URL'),
        username: configService.get('NEO4J_USERNAME'),
        password: configService.get('NEO4J_PASSWORD'),
        mode: configService.get('NODE_ENV')
      })
    }),
    SqlModule.forRootAsync()
  ],
  providers: [Neo4jCapabilitiesService],
  exports: [Neo4jCapabilitiesService]
})
export class DatabaseModule implements OnModuleInit {
  constructor(
    private readonly neogmaService: NeogmaService,
    private readonly configService: ConfigService
  ) {}

  async onModuleInit() {
    if (isDevMode()) {
      const { hostname } = new URL(this.configService.get('NEO4J_URL'))
      if (hostname === 'localhost' || hostname === '127.0.0.1') {
        Logger.log('Checking if DB is ready...')
        const healthCheckUrl = `http://${hostname}:7474`
        await fetchRetry(healthCheckUrl, 5000, 15)
        Logger.log('DB is ready')
      }
    }

    const session = this.neogmaService.createSession('database-seed')

    // Drop the plain composite index before creating the uniqueness constraint that
    // covers the same properties — Neo4j requires these to be in separate transactions.
    const dropTx = session.beginTransaction({ timeout: DEFAULT_TRANSACTION_TIMEOUT_MS })
    try {
      await dropTx.run(`DROP INDEX index_property_mergerer IF EXISTS`)
      await dropTx.commit()
    } catch (error) {
      Logger.log('Warning: could not drop index_property_mergerer', error)
      await dropTx.rollback()
    }

    const transaction = session.beginTransaction({ timeout: DEFAULT_TRANSACTION_TIMEOUT_MS })
    try {
      const constraints = [
        `CREATE CONSTRAINT constraint_record_id IF NOT EXISTS FOR (record:${RUSHDB_LABEL_RECORD}) REQUIRE record.${RUSHDB_KEY_ID} IS UNIQUE`,
        `CREATE CONSTRAINT constraint_property_id IF NOT EXISTS FOR (property:${RUSHDB_LABEL_PROPERTY}) REQUIRE property.id IS UNIQUE`,
        `CREATE CONSTRAINT constraint_property_uniqueness IF NOT EXISTS FOR (p:${RUSHDB_LABEL_PROPERTY}) REQUIRE (p.name, p.type, p.projectId, p.metadata) IS UNIQUE`
      ]

      const indexes = [
        `CREATE INDEX index_record_id IF NOT EXISTS FOR (n:${RUSHDB_LABEL_RECORD}) ON (n.${RUSHDB_KEY_ID})`,
        `CREATE INDEX index_record_projectid IF NOT EXISTS FOR (n:${RUSHDB_LABEL_RECORD}) ON (n.${RUSHDB_KEY_PROJECT_ID})`,
        `CREATE INDEX index_property_name IF NOT EXISTS FOR (n:${RUSHDB_LABEL_PROPERTY}) ON (n.name)`
      ]

      Logger.log('Creating Neo4j constraints...')
      for (const constraint of constraints) {
        await transaction.run(constraint)
      }

      Logger.log('Creating Neo4j indexes...')
      for (const index of indexes) {
        await transaction.run(index)
      }
    } catch (error) {
      Logger.log('Initializing RushDB failed.', error)
      await transaction.rollback()
    } finally {
      Logger.log('Initializing RushDB finished.')
      if (transaction.isOpen()) {
        await transaction.commit()
        await transaction.close()
      }
      await this.neogmaService.closeSession(session)
    }
  }
}
