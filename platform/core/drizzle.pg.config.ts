import type { Config } from 'drizzle-kit'

const url = process.env.SQL_DB_URL ?? 'postgresql://localhost:5432/rushdb'

export default {
  schema: './src/database/sql/schema/pg.schema.ts',
  out: './src/database/sql/migrations/pg',
  dialect: 'postgresql',
  dbCredentials: {
    url,
    ssl: { rejectUnauthorized: false }
  }
} satisfies Config
