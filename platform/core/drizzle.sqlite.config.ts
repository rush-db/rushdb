import type { Config } from 'drizzle-kit'

export default {
  schema: './src/database/sql/schema/sqlite.schema.ts',
  out: './src/database/sql/migrations/sqlite',
  dialect: 'sqlite',
  dbCredentials: {
    url: process.env.SQL_DB_PATH ?? './rushdb.db'
  }
} satisfies Config
