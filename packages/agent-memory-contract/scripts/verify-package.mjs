import { spawnSync } from 'node:child_process'
import { mkdtempSync, readdirSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const packageDirectory = dirname(dirname(fileURLToPath(import.meta.url)))
const packDirectory = mkdtempSync(join(tmpdir(), 'rushdb-agent-memory-contract-'))
const requiredFiles = [
  'package/README.md',
  'package/dist/index.js',
  'package/dist/index.d.ts',
  'package/schema/agent-memory-event.v1.schema.json',
  'package/fixtures/conformance.v1.json'
]

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: packageDirectory,
    encoding: 'utf8',
    ...options
  })
  if (result.status !== 0) {
    process.stderr.write(result.stdout)
    process.stderr.write(result.stderr)
    process.exit(result.status ?? 1)
  }
  return result.stdout
}

try {
  run('npm', ['pack', '--pack-destination', packDirectory], {
    env: { ...process.env, npm_config_cache: join(packDirectory, 'npm-cache') }
  })
  const tarball = readdirSync(packDirectory).find((entry) => entry.endsWith('.tgz'))
  if (!tarball) throw new Error('npm pack did not create a tarball')

  const files = new Set(
    run('tar', ['-tzf', join(packDirectory, tarball)])
      .trim()
      .split('\n')
  )
  const missing = requiredFiles.filter((file) => !files.has(file))
  if (missing.length > 0) {
    throw new Error(`Agent memory contract package is missing: ${missing.join(', ')}`)
  }

  console.log(`Verified ${tarball}: ${requiredFiles.length} required files are present`)
} finally {
  rmSync(packDirectory, { recursive: true, force: true })
}
