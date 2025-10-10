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
import { fileURLToPath } from 'url'
import { dirname, resolve } from 'path'
import fs from 'fs'

// Attempt fallback resolution for .env when launched from a different CWD (e.g., repo root)
function ensureEnvLoaded() {
  // If both are already present (loaded by dotenv or environment), skip fallback
  if (process.env.RUSHDB_API_KEY && process.env.RUSHDB_API_URL) return

  const here = dirname(fileURLToPath(import.meta.url)) // .../packages/mcp-server/build/util
  const candidatePaths = [
    resolve(process.cwd(), '.env'), // user CWD
    resolve(here, '../../.env'), // package root (from build/util)
    resolve(here, '../.env') // build folder parent (rare edge)
  ]

  for (const p of candidatePaths) {
    try {
      if (fs.existsSync(p)) {
        // Dynamically load without re-importing dotenv/config (which already ran); parse manually
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
        // Stop once we've loaded what we need
        if (process.env.RUSHDB_API_KEY && process.env.RUSHDB_API_URL) break
      }
    } catch (e) {
      // Non-fatal; continue to next candidate
    }
  }
}

ensureEnvLoaded()

const token = process.env.RUSHDB_API_KEY
const url = process.env.RUSHDB_API_URL || 'https://api.rushdb.com/api/v1'

if (!token) {
  throw new Error(
    'RUSHDB_API_KEY environment variable is required. Set it in a .env file (packages/mcp-server/.env) or export it before running the server.'
  )
}

const RushDBCtor: any =
  (RushDBImport as any)?.default || (RushDBImport as any)?.RushDB || (RushDBImport as any)

export const db = new RushDBCtor(token, { url })
