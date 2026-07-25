import { createHmac } from 'node:crypto'

const required = (name) => {
  const value = process.env[name]
  if (!value) throw new Error(`Missing required environment variable: ${name}`)
  return value
}

const version = required('RELEASE_VERSION')

// Packages no longer move in lockstep on patch releases, so report what was
// actually published. PUBLISHED_PACKAGES is changesets' `publishedPackages`
// output ([{ name, version }]) and is empty for a core-only release.
const parsePublished = () => {
  try {
    const parsed = JSON.parse(process.env.PUBLISHED_PACKAGES || '[]')
    if (Array.isArray(parsed) && parsed.length) {
      return parsed.map(({ name, version }) => ({ name, version }))
    }
  } catch {
    /* fall through to the version-derived list */
  }
  return [
    { name: '@rushdb/javascript-sdk', version },
    { name: '@rushdb/mcp-server', version }
  ]
}

const body = JSON.stringify({
  event: 'release.completed',
  repository: process.env.GITHUB_REPOSITORY ?? 'rush-db/rushdb',
  version,
  tag: `v${version}`,
  platformVersion: process.env.PLATFORM_VERSION || version,
  sha: required('RELEASE_SHA'),
  workflowUrl: required('RELEASE_RUN_URL'),
  packages: parsePublished(),
  occurredAt: new Date().toISOString()
})
const timestamp = String(Math.floor(Date.now() / 1000))
const signature = `sha256=${createHmac('sha256', required('CONTENT_WEBHOOK_SECRET')).update(`${timestamp}.${body}`).digest('hex')}`
const response = await fetch(required('CONTENT_WEBHOOK_URL'), {
  method: 'POST',
  headers: {
    'content-type': 'application/json',
    'x-content-timestamp': timestamp,
    'x-content-signature': signature
  },
  body
})
if (!response.ok) throw new Error(`Content webhook failed (${response.status}): ${await response.text()}`)
console.log(await response.text())
