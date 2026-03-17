// Copyright Collect Software, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import 'dotenv/config'
import RushDBImport from '@rushdb/javascript-sdk'
import { AsyncLocalStorage } from 'node:async_hooks'
import { fileURLToPath } from 'url'
import { dirname, resolve } from 'path'
import fs from 'fs'

// ─── .env fallback resolution ────────────────────────────────────────────────

function ensureEnvLoaded() {
  if (process.env.RUSHDB_API_KEY && process.env.RUSHDB_API_URL) return

  const here = dirname(fileURLToPath(import.meta.url)) // .../build/util
  const candidatePaths = [
    resolve(process.cwd(), '.env'),
    resolve(here, '../../.env'),
    resolve(here, '../.env')
  ]

  for (const p of candidatePaths) {
    try {
      if (fs.existsSync(p)) {
        const content = fs.readFileSync(p, 'utf8')
        for (const line of content.split(/\r?\n/)) {
          if (!line || line.trim().startsWith('#')) continue
          const idx = line.indexOf('=')
          if (idx === -1) continue
          const key = line.slice(0, idx).trim()
          if (key !== 'RUSHDB_API_KEY' && key !== 'RUSHDB_API_URL') continue
          const value = line.slice(idx + 1).trim()
          if (value) process.env[key] ||= value
        }
        if (process.env.RUSHDB_API_KEY && process.env.RUSHDB_API_URL) break
      }
    } catch (e) {
      // Non-fatal
    }
  }
}

ensureEnvLoaded()

// ─── RushDB constructor ───────────────────────────────────────────────────────

const RushDBCtor: any =
  (RushDBImport as any)?.default || (RushDBImport as any)?.RushDB || (RushDBImport as any)

// ─── Per-request async context (HTTP / OAuth mode) ───────────────────────────

export interface RequestContext {
  db: any
  userId?: string
  scopes?: string[]
  projectId?: string
}

/**
 * AsyncLocalStorage for per-request execution contexts in HTTP mode.
 * In STDIO mode this is never populated.
 */
export const requestContext = new AsyncLocalStorage<RequestContext>()

// ─── STDIO singleton ──────────────────────────────────────────────────────────

let _stdioDb: any | null = null

/**
 * Returns the STDIO-mode RushDB client backed by RUSHDB_API_KEY env var.
 * Throws if the env var is not set.
 */
export function getStdioDb(): any {
  if (!_stdioDb) {
    const token = process.env.RUSHDB_API_KEY
    if (!token) {
      throw new Error(
        'RUSHDB_API_KEY environment variable is required. Set it in a .env file or export it before running the server.'
      )
    }
    const url = process.env.RUSHDB_API_URL || 'https://api.rushdb.com/api/v1'
    _stdioDb = new RushDBCtor(token, { url })
  }
  return _stdioDb
}

// ─── Per-request or fallback client ──────────────────────────────────────────

/**
 * Returns the RushDB client for the current execution context.
 *
 * - In HTTP mode: returns the per-request db injected via `requestContext.run()`.
 * - In stdio mode (no context): returns the shared STDIO singleton.
 */
export function getDb(): any {
  const ctx = requestContext.getStore()
  if (ctx?.db) return ctx.db
  return getStdioDb()
}

/**
 * Creates a new RushDB client for an explicit token (used in HTTP / OAuth mode).
 */
export function createDbForToken(token: string, url?: string): any {
  const resolvedUrl = url || process.env.RUSHDB_API_URL || 'https://api.rushdb.com/api/v1'
  return new RushDBCtor(token, { url: resolvedUrl })
}

// ─── Backwards-compatible `db` export ────────────────────────────────────────
// All existing `import { db } from '../util/db.js'` calls transparently route
// through getDb(), which checks the per-request context first, then falls
// back to the STDIO singleton.  No changes needed in individual tool files.

export const db = new Proxy({} as any, {
  get(_target, prop) {
    return getDb()[prop]
  },
  set(_target, prop, value) {
    getDb()[prop] = value
    return true
  }
})
