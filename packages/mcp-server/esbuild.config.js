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

import { build, context } from 'esbuild'
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

// Simple plugin to make output executable & log results
const chmodAndLogPlugin = {
  name: 'chmod-and-log',
  setup(build) {
    build.onEnd((result) => {
      try {
        if (fs.existsSync('build/index.js')) {
          fs.chmodSync('build/index.js', '755')
        }
      } catch (e) {
        console.warn('Failed to chmod output:', e)
      }
      if (result.errors.length === 0) {
        console.log('Build succeeded', new Date().toLocaleTimeString())
      } else {
        console.error('Build ended with errors.')
      }
    })
  }
}

buildOptions.plugins = [chmodAndLogPlugin]

async function runBuild() {
  try {
    if (isWatch) {
      // New esbuild 0.25+ context watch API
      const ctx = await context(buildOptions)
      await ctx.watch()
      console.log('Watch mode started (esbuild context). Press Ctrl+C to exit.')
    } else {
      // One-time build
      await build(buildOptions)
      console.log(`Build completed successfully in ${isProd ? 'production' : 'development'} mode!`)
    }
  } catch (error) {
    console.error('Build failed:', error)
    process.exit(1)
  }
}

runBuild()
