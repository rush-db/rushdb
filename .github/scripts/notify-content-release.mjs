import { createHmac } from 'node:crypto'

const required = (name) => {
  const value = process.env[name]
  if (!value) throw new Error(`Missing required environment variable: ${name}`)
  return value
}

const version = required('RELEASE_VERSION')
const body = JSON.stringify({
  event: 'release.completed',
  repository: process.env.GITHUB_REPOSITORY ?? 'rush-db/rushdb',
  version,
  tag: `v${version}`,
  sha: required('RELEASE_SHA'),
  workflowUrl: required('RELEASE_RUN_URL'),
  packages: [
    { name: '@rushdb/javascript-sdk', version },
    { name: '@rushdb/mcp-server', version }
  ],
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
