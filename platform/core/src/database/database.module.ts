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
import {
  RUSHDB_LABEL_PROJECT,
  RUSHDB_LABEL_TOKEN,
  RUSHDB_LABEL_USER,
  RUSHDB_LABEL_WORKSPACE
} from '@/dashboard/common/constants'
import { INeogmaConfig } from '@/database/neogma/neogma-config.interface'
import { NeogmaModule } from '@/database/neogma/neogma.module'
import { NeogmaService } from '@/database/neogma/neogma.service'

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
    })
  ]
})
export class DatabaseModule implements OnModuleInit {
  constructor(
    private readonly neogmaService: NeogmaService,
    private readonly configService: ConfigService
  ) {}

  async onModuleInit() {
    if (isDevMode()) {
      Logger.log('Checking if DB is ready...')
      const { hostname } = new URL(this.configService.get('NEO4J_URL'))
      const healthCheckUrl = `http://${hostname}:7474`
      await fetchRetry(healthCheckUrl, 5000, 15)
      Logger.log('DB is ready')
    }

    const session = this.neogmaService.createSession()
    const transaction = session.beginTransaction()
    try {
      const constraints = [
        `CREATE CONSTRAINT constraint_user_login IF NOT EXISTS FOR (user:${RUSHDB_LABEL_USER}) REQUIRE user.login IS UNIQUE`,
        `CREATE CONSTRAINT constraint_user_id IF NOT EXISTS FOR (user:${RUSHDB_LABEL_USER}) REQUIRE user.id IS UNIQUE`,
        `CREATE CONSTRAINT constraint_token_id IF NOT EXISTS FOR (token:${RUSHDB_LABEL_TOKEN}) REQUIRE token.id IS UNIQUE`,
        `CREATE CONSTRAINT constraint_project_id IF NOT EXISTS FOR (project:${RUSHDB_LABEL_PROJECT}) REQUIRE project.id IS UNIQUE`,
        `CREATE CONSTRAINT constraint_workspace_id IF NOT EXISTS FOR (workspace:${RUSHDB_LABEL_WORKSPACE}) REQUIRE workspace.id IS UNIQUE`,
        `CREATE CONSTRAINT constraint_record_id IF NOT EXISTS FOR (record:${RUSHDB_LABEL_RECORD}) REQUIRE record.${RUSHDB_KEY_ID} IS UNIQUE`,
        `CREATE CONSTRAINT constraint_property_id IF NOT EXISTS FOR (property:${RUSHDB_LABEL_PROPERTY}) REQUIRE property.id IS UNIQUE`
      ]

      const indexes = [
        `CREATE INDEX index_record_id IF NOT EXISTS FOR (n:${RUSHDB_LABEL_RECORD}) ON (n.${RUSHDB_KEY_ID})`,
        `CREATE INDEX index_record_projectid IF NOT EXISTS FOR (n:${RUSHDB_LABEL_RECORD}) ON (n.${RUSHDB_KEY_PROJECT_ID})`,
        `CREATE INDEX index_property_name IF NOT EXISTS FOR (n:${RUSHDB_LABEL_PROPERTY}) ON (n.name)`,
        `CREATE INDEX index_property_mergerer IF NOT EXISTS FOR (n:${RUSHDB_LABEL_PROPERTY}) ON (n.name, n.type, n.projectId, n.metadata)`
      ]

      Logger.log('Creating constraints...')
      for (const constraint of constraints) {
        await transaction.run(constraint)
      }

      Logger.log('Creating indexes...')
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
      await session.close()
    }
  }
}
