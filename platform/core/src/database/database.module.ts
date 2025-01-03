import { Global, Module, OnModuleInit } from '@nestjs/common'
import { ConfigModule, ConfigService } from '@nestjs/config'

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
  constructor(private readonly neogmaService: NeogmaService) {}

  async onModuleInit() {
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

      for (const constraint of constraints) {
        await transaction.run(constraint)
      }
      for (const index of indexes) {
        await transaction.run(index)
      }
    } catch (error) {
      await transaction.rollback()
    } finally {
      if (transaction.isOpen()) {
        await transaction.commit()
        await transaction.close()
      }
      await session.close()
    }
  }
}
