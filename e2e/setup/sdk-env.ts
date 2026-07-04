/**
 * Runs before each test file (setupFilesAfterEnv). Feeds the SDK e2e tests their
 * RUSHDB_API_KEY / RUSHDB_API_URL:
 *
 * 1. Harness-provisioned credentials (.sdk-env.json, written by global-setup) win —
 *    they are the only ones guaranteed to match the stack the harness booted.
 * 2. Otherwise packages/javascript-sdk/.env is loaded as a fallback so the SDK suites
 *    can still be pointed at an arbitrary instance the way they historically were.
 *
 * Tests skip themselves gracefully when RUSHDB_API_KEY ends up unset.
 */
import dotenv from 'dotenv'
import { existsSync, readFileSync } from 'fs'
import { resolve } from 'path'

import { REPO_ROOT, SDK_ENV_FILE } from './env'

if (existsSync(SDK_ENV_FILE)) {
  const { apiKey, apiUrl, customDbApiKey, embeddingEnabled } = JSON.parse(
    readFileSync(SDK_ENV_FILE, 'utf-8')
  )
  process.env.RUSHDB_API_KEY = apiKey
  process.env.RUSHDB_API_URL = apiUrl
  process.env.RUSHDB_E2E_EMBEDDINGS = String(Boolean(embeddingEnabled))
  if (customDbApiKey) {
    // Raw-query suites need a project attached to an external Neo4j.
    process.env.RUSHDB_E2E_CUSTOMDB_API_KEY = customDbApiKey
  }
} else {
  dotenv.config({ path: resolve(REPO_ROOT, 'packages/javascript-sdk/.env') })
}
