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

import { build } from 'esbuild'
import { fileURLToPath } from 'url'
import { dirname, resolve } from 'path'
import fs from 'fs'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)

// Check if watch mode is enabled
const isWatch = process.argv.includes('--watch')
// Check if production mode is enabled
const isProd = process.argv.includes('--prod')

const buildOptions = {
  entryPoints: ['index.ts'],
  bundle: true,
  platform: 'node',
  target: 'node18',
  format: 'esm',
  outfile: 'build/index.js',
  banner: {
    js: '#!/usr/bin/env node'
  },
  external: [
    // External packages that should not be bundled
    '@rushdb/javascript-sdk',
    '@modelcontextprotocol/sdk',
    'jsonschema',
    'dotenv'
  ],
  minify: isProd,
  sourcemap: !isProd,
  logLevel: 'info'
}

async function runBuild() {
  try {
    if (isWatch) {
      // Watch mode
      const ctx = await build({
        ...buildOptions,
        watch: {
          onRebuild(error, result) {
            if (error) {
              console.error('Watch build failed:', error)
            } else {
              // Make the output file executable
              fs.chmodSync('build/index.js', '755')
              console.log('Watch build succeeded:', new Date().toLocaleTimeString())
            }
          }
        }
      })

      // Make the output file executable after initial build
      fs.chmodSync('build/index.js', '755')
      console.log('Watch mode started, waiting for changes...')

      // Keep the process running
      process.stdin.on('close', () => {
        ctx.stop()
        process.exit(0)
      })
    } else {
      // One-time build
      await build(buildOptions)

      // Make the output file executable
      fs.chmodSync('build/index.js', '755')

      console.log(`Build completed successfully in ${isProd ? 'production' : 'development'} mode!`)
    }
  } catch (error) {
    console.error('Build failed:', error)
    process.exit(1)
  }
}

runBuild()
