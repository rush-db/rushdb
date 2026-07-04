import { DynamicModule, Global, Inject, Logger, Module, OnModuleInit } from '@nestjs/common'
import { ConfigModule, ConfigService } from '@nestjs/config'

import { existsSync } from 'node:fs'
import { resolve } from 'node:path'

import { SQL_DB, SQL_SCHEMA } from './sql.constants'
import { SqlService } from './sql.service'

export type SqlDbType = 'sqlite' | 'postgres'

export interface SqlModuleOptions {
  type: SqlDbType
  /** SQLite only – path to the .db file (default: ./rushdb.db) */
  path?: string
  /** Postgres only – connection string */
  url?: string
}

@Global()
@Module({})
export class SqlModule implements OnModuleInit {
  private readonly logger = new Logger(SqlModule.name)

  constructor(
    @Inject(SQL_DB) private readonly db: any,
    @Inject(SQL_SCHEMA) private readonly schema: any,
    @Inject('SQL_DB_TYPE') private readonly dbType: SqlDbType,
    @Inject('SQL_MIGRATIONS_FOLDER') private readonly migrationsFolder: string
  ) {}

  async onModuleInit() {
    try {
      if (this.dbType === 'sqlite') {
        const { migrate } = await import('drizzle-orm/better-sqlite3/migrator')
        migrate(this.db, { migrationsFolder: this.migrationsFolder })
      } else {
        const { migrate } = await import('drizzle-orm/node-postgres/migrator')
        await migrate(this.db, { migrationsFolder: this.migrationsFolder })
      }
      this.logger.log(`SQL migrations applied (${this.dbType})`)
    } catch (err) {
      this.logger.error('SQL migration failed', err)
      throw err
    }
  }

  static forRootAsync(): DynamicModule {
    return {
      module: SqlModule,
      imports: [ConfigModule],
      providers: [
        {
          provide: 'SQL_MIGRATIONS_FOLDER',
          inject: [ConfigService],
          useFactory: (configService: ConfigService): string => {
            const dbType = configService.get<string>('SQL_DB_TYPE') ?? 'sqlite'
            const dialect = dbType === 'postgres' ? 'pg' : 'sqlite'
            // __dirname is dist/database/sql in compiled output; fall back to src for dev watch mode
            const distPath = resolve(__dirname, 'migrations', dialect)
            const srcPath = resolve(process.cwd(), 'src/database/sql/migrations', dialect)
            const folder = existsSync(distPath) ? distPath : srcPath
            return folder
          }
        },
        {
          provide: 'SQL_DB_TYPE',
          inject: [ConfigService],
          useFactory: (configService: ConfigService): SqlDbType => {
            const raw = configService.get<string>('SQL_DB_TYPE') ?? 'sqlite'
            return raw === 'postgres' ? 'postgres' : 'sqlite'
          }
        },
        {
          provide: SQL_SCHEMA,
          inject: ['SQL_DB_TYPE'],
          useFactory: async (dbType: SqlDbType) => {
            if (dbType === 'postgres') {
              const { pgSchema } = await import('./schema/pg.schema')
              return pgSchema
            }
            const { sqliteSchema } = await import('./schema/sqlite.schema')
            return sqliteSchema
          }
        },
        {
          provide: SQL_DB,
          inject: [ConfigService, 'SQL_DB_TYPE', SQL_SCHEMA],
          useFactory: async (configService: ConfigService, dbType: SqlDbType, schema: any) => {
            if (dbType === 'postgres') {
              const { Pool } = await import('pg')
              const { drizzle } = await import('drizzle-orm/node-postgres')
              const connectionString =
                configService.get<string>('SQL_DB_URL') ?? configService.get<string>('DATABASE_URL')
              if (!connectionString) {
                throw new Error('SQL_DB_URL or DATABASE_URL must be set when SQL_DB_TYPE=postgres')
              }
              const sslEnv = configService.get<string>('SQL_DB_SSL')
              const isLocal =
                connectionString?.includes('localhost') || connectionString?.includes('127.0.0.1')
              const ssl = sslEnv === 'false' || (!sslEnv && isLocal) ? false : { rejectUnauthorized: false }
              const pool = new Pool({ connectionString, ssl })
              // Without a handler, an error on an idle client (e.g. the server dropping a
              // connection: 57P01 "terminating connection due to administrator command") is
              // emitted as an unhandled 'error' event and crashes the whole process. The pool
              // discards the broken client and opens a fresh one on the next query.
              pool.on('error', (error) => {
                Logger.error('[SqlModule] Idle Postgres client error (connection dropped)', error)
              })
              return drizzle(pool, { schema })
            }

            // eslint-disable-next-line @typescript-eslint/no-require-imports
            const Database = require('better-sqlite3')
            const { drizzle } = await import('drizzle-orm/better-sqlite3')
            const dbPath = configService.get<string>('SQL_DB_PATH') ?? './rushdb.db'
            const client = new Database(dbPath)
            return drizzle(client, { schema })
          }
        },
        SqlService
      ],
      exports: [SQL_DB, SQL_SCHEMA, 'SQL_DB_TYPE', SqlService]
    }
  }
}
