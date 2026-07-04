import { execSync, spawn } from 'child_process'
import { openSync, writeFileSync } from 'fs'

import { provisionApiKey } from './bootstrap'
import {
  ADMIN_LOGIN,
  ADMIN_PASSWORD,
  BASE_URL,
  COMPOSE_FILE,
  COMPOSE_PROJECT,
  CORE_DIR,
  REPO_ROOT,
  SDK_ENV_FILE,
  SERVER_ENV,
  SERVER_LOG_FILE,
  STATE_FILE
} from './env'

const HEALTH_TIMEOUT_MS = 120_000

const waitForHealth = async (url: string, timeoutMs: number): Promise<void> => {
  const deadline = Date.now() + timeoutMs
  let lastError: unknown
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`${url}/health`)
      if (response.ok) {
        return
      }
      lastError = new Error(`health returned ${response.status}`)
    } catch (e) {
      lastError = e
    }
    await new Promise((r) => setTimeout(r, 1000))
  }
  throw new Error(`Platform did not become healthy at ${url}/health within ${timeoutMs}ms: ${lastError}`)
}

/**
 * Provisions a shared project + API key for the SDK e2e suites (e2e/sdk/*), which read
 * RUSHDB_API_KEY / RUSHDB_API_URL via setup/sdk-env.ts. Failure is non-fatal on external
 * stacks (admin login may be unavailable there) — the SDK suites then fall back to
 * packages/javascript-sdk/.env or skip themselves.
 */
const provisionSdkCredentials = async ({ provisionedStack }: { provisionedStack: boolean }) => {
  try {
    const run = Date.now().toString(36)
    const { apiKey } = await provisionApiKey({
      baseUrl: BASE_URL,
      login: ADMIN_LOGIN,
      password: ADMIN_PASSWORD,
      projectName: `sdk-e2e-${run}`
    })

    // The raw-query API requires a project attached to an external Neo4j. On the
    // self-provisioned stack we know the Neo4j coordinates, so create a second project
    // with customDb for those suites; on an external stack we can't guess them.
    let customDbApiKey: string | undefined
    if (provisionedStack) {
      customDbApiKey = (
        await provisionApiKey({
          baseUrl: BASE_URL,
          login: ADMIN_LOGIN,
          password: ADMIN_PASSWORD,
          projectName: `sdk-e2e-customdb-${run}`,
          customDb: {
            url: SERVER_ENV.NEO4J_URL,
            username: SERVER_ENV.NEO4J_USERNAME,
            password: SERVER_ENV.NEO4J_PASSWORD
          }
        })
      ).apiKey
    }

    // Suites that depend on server-side embedding generation skip themselves when the
    // stack has no embedding provider configured.
    const settings = await fetch(`${BASE_URL}/api/v1/settings`)
      .then((r) => r.json())
      .catch(() => ({}))
    const embeddingEnabled = Boolean((settings as any)?.data?.embeddingEnabled)

    writeFileSync(
      SDK_ENV_FILE,
      JSON.stringify({ apiKey, apiUrl: BASE_URL, customDbApiKey, embeddingEnabled })
    )
  } catch (error) {
    console.warn(`[e2e] SDK credential bootstrap failed (SDK suites may skip): ${error}`)
  }
}

export default async function globalSetup() {
  // Target an externally managed stack (e.g. local dev server) — nothing to provision.
  if (process.env.E2E_BASE_URL) {
    writeFileSync(STATE_FILE, JSON.stringify({ external: true }))
    await waitForHealth(BASE_URL, 10_000)
    await provisionSdkCredentials({ provisionedStack: false })
    return
  }

  console.log('\n[e2e] starting Neo4j + Postgres containers...')
  execSync(`docker compose -p ${COMPOSE_PROJECT} -f ${COMPOSE_FILE} up -d --wait`, {
    stdio: 'inherit'
  })

  console.log('[e2e] building platform/core...')
  execSync('pnpm --filter rushdb-core build', { cwd: REPO_ROOT, stdio: 'inherit' })

  console.log('[e2e] starting platform/core server...')
  const logFd = openSync(SERVER_LOG_FILE, 'w')
  const server = spawn('node', ['dist/main.js'], {
    cwd: CORE_DIR,
    env: { ...process.env, ...SERVER_ENV },
    stdio: ['ignore', logFd, logFd],
    detached: true
  })
  server.unref()

  writeFileSync(STATE_FILE, JSON.stringify({ external: false, serverPid: server.pid }))

  try {
    await waitForHealth(BASE_URL, HEALTH_TIMEOUT_MS)
  } catch (error) {
    try {
      execSync(`tail -50 ${SERVER_LOG_FILE}`, { stdio: 'inherit' })
    } catch {
      /* empty */
    }
    throw error
  }
  console.log('[e2e] platform is up')
  await provisionSdkCredentials({ provisionedStack: true })
}
