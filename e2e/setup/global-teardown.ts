import { execSync } from 'child_process'
import { existsSync, readFileSync, unlinkSync } from 'fs'

import { COMPOSE_FILE, COMPOSE_PROJECT, SDK_ENV_FILE, STATE_FILE } from './env'

export default async function globalTeardown() {
  if (existsSync(SDK_ENV_FILE)) {
    unlinkSync(SDK_ENV_FILE)
  }
  if (!existsSync(STATE_FILE)) {
    return
  }

  const state = JSON.parse(readFileSync(STATE_FILE, 'utf-8')) as {
    external?: boolean
    serverPid?: number
  }
  unlinkSync(STATE_FILE)

  if (state.external) {
    return
  }

  if (state.serverPid) {
    try {
      process.kill(state.serverPid, 'SIGTERM')
    } catch {
      /* already exited */
    }
  }

  execSync(`docker compose -p ${COMPOSE_PROJECT} -f ${COMPOSE_FILE} down -v`, { stdio: 'inherit' })
}
