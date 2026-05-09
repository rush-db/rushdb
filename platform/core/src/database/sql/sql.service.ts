import { Inject, Injectable } from '@nestjs/common'
import { and, eq, SQL } from 'drizzle-orm'

import { SQL_DB, SQL_SCHEMA } from './sql.constants'

import type { SqlDbType } from './sql.module'

/**
 * Thin wrapper that provides the Drizzle `db` and `schema` for injection
 * in repositories. The `db` is typed loosely as `any` because it can be
 * either a BetterSQLite3Database or NodePgDatabase depending on runtime config.
 *
 * All column/table types are inferred from the SQLite schema (structural
 * equivalence means they apply to the PG schema too).
 */
@Injectable()
export class SqlService {
  constructor(
    @Inject(SQL_DB) public readonly db: any,
    @Inject(SQL_SCHEMA) public readonly schema: any,
    @Inject('SQL_DB_TYPE') public readonly dbType: SqlDbType
  ) {}

  get tables() {
    return this.schema as import('./schema/sqlite.schema').SqliteSchema
  }

  /** True when running against PostgreSQL. */
  get isPostgres() {
    return this.dbType === 'postgres'
  }
}
