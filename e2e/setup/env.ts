import { resolve } from 'path'

export const REPO_ROOT = resolve(__dirname, '..', '..')
export const E2E_DIR = resolve(REPO_ROOT, 'e2e')
export const CORE_DIR = resolve(REPO_ROOT, 'platform', 'core')

export const COMPOSE_FILE = resolve(E2E_DIR, 'docker-compose.yml')
export const COMPOSE_PROJECT = 'rushdb-e2e'

export const STATE_FILE = resolve(E2E_DIR, '.e2e-state.json')
export const SERVER_LOG_FILE = resolve(E2E_DIR, '.server.log')
/** Harness-provisioned SDK credentials, consumed by setup/sdk-env.ts. */
export const SDK_ENV_FILE = resolve(E2E_DIR, '.sdk-env.json')

export const APP_PORT = 3390
export const BASE_URL = process.env.E2E_BASE_URL ?? `http://localhost:${APP_PORT}`

export const ADMIN_LOGIN = process.env.E2E_LOGIN ?? 'admin'
export const ADMIN_PASSWORD = process.env.E2E_PASSWORD ?? 'e2e-password'

/** Environment for the platform/core server process. */
export const SERVER_ENV: Record<string, string> = {
  RUSHDB_PORT: String(APP_PORT),
  RUSHDB_SELF_HOSTED: 'true',
  RUSHDB_LOGIN: ADMIN_LOGIN,
  RUSHDB_PASSWORD: ADMIN_PASSWORD,
  // Must be exactly 32 characters
  RUSHDB_AES_256_ENCRYPTION_KEY: 'e2e0123456789abcdef0123456789abc',
  NEO4J_URL: 'bolt://localhost:7688',
  NEO4J_USERNAME: 'neo4j',
  NEO4J_PASSWORD: 'e2e-password',
  SQL_DB_TYPE: 'postgres',
  SQL_DB_URL: 'postgresql://rushdb:password@localhost:5434/rushdb-e2e'
}
