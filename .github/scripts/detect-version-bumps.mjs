// Detects which workspace packages had their version changed by the push that
// triggered this workflow.
//
// This is the release gate. A feature/fix branch merged into main changes no
// package.json version, so nothing is built or deployed. Merging the changesets
// "version packages" PR does change versions — that is the only event that ships
// images to Docker Hub and ECS.
//
// Versions are read out of git objects (`git show <ref>:<path>`), never off disk,
// so the result is unaffected by `changeset version` mutating the working tree
// later in the same job.
//
// Env:
//   BASE_SHA        state of main before this push (github.event.before)
//   HEAD_SHA        commit being released (github.sha); defaults to HEAD
//   FORCE_PLATFORM  'true' → report the platform image as changed regardless
//   FORCE_MCP       'true' → report the mcp image as changed regardless

import { execFileSync } from 'node:child_process'
import { appendFileSync } from 'node:fs'

const TRACKED = [
  { key: 'core', path: 'platform/core/package.json' },
  { key: 'dashboard', path: 'platform/dashboard/package.json' },
  { key: 'sdk', path: 'packages/javascript-sdk/package.json' },
  { key: 'mcp', path: 'packages/mcp-server/package.json' }
]

const git = (...args) =>
  execFileSync('git', args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim()

const commitExists = (ref) => {
  if (!ref) return false
  try {
    git('cat-file', '-e', `${ref}^{commit}`)
    return true
  } catch {
    return false
  }
}

// Returns null when the file does not exist at that ref (new package, shallow
// history, unreachable base) — treated as "changed".
const versionAt = (ref, path) => {
  try {
    return JSON.parse(git('show', `${ref}:${path}`)).version ?? null
  } catch {
    return null
  }
}

const head = commitExists(process.env.HEAD_SHA) ? process.env.HEAD_SHA : 'HEAD'

// github.event.before is absent on workflow_dispatch and all-zeroes for a
// freshly created ref; fall back to the release commit's first parent, which is
// main-before-merge for a merge commit and the previous commit otherwise.
let base = process.env.BASE_SHA
if (!commitExists(base) || /^0+$/.test(base ?? '')) {
  base = commitExists(`${head}^1`) ? `${head}^1` : null
}

const forcePlatform = process.env.FORCE_PLATFORM === 'true'
const forceMcp = process.env.FORCE_MCP === 'true'

const result = {}
const rows = []

for (const { key, path } of TRACKED) {
  const current = versionAt(head, path)
  const previous = base ? versionAt(base, path) : null
  const changed = current !== null && current !== previous

  result[`${key}_version`] = current ?? ''
  result[`${key}_changed`] = String(changed)
  rows.push(
    `| \`${path}\` | ${previous ?? '—'} | ${current ?? '—'} | ${changed ? '**bumped**' : 'unchanged'} |`
  )
}

// The platform image bundles core + dashboard, and is tagged with core's
// version. core and dashboard are a `fixed` changesets group, so they always
// bump together — the tag therefore always advances when the image content
// changes and no published tag is ever overwritten. An SDK-only release still
// reaches this image: `updateInternalDependencies: patch` bumps the dashboard
// (a workspace dependent of the SDK), which drags core along.
const platformChanged = result.core_changed === 'true' || result.dashboard_changed === 'true' || forcePlatform
const mcpChanged = result.mcp_changed === 'true' || forceMcp

result.platform_changed = String(platformChanged)
result.platform_version = result.core_version
result.mcp_image_changed = String(mcpChanged)
result.any_changed = String(platformChanged || mcpChanged)

if (process.env.GITHUB_OUTPUT) {
  appendFileSync(
    process.env.GITHUB_OUTPUT,
    Object.entries(result)
      .map(([k, v]) => `${k}=${v}`)
      .join('\n') + '\n'
  )
}

const summary = [
  `### Release gate`,
  '',
  `Comparing \`${base ?? 'unknown'}\` → \`${head}\``,
  '',
  '| package | before | after | |',
  '| --- | --- | --- | --- |',
  ...rows,
  '',
  `- platform image (\`rushdb/platform:${result.platform_version || '?'}\`): **${platformChanged ? 'build + deploy' : 'skip'}**${forcePlatform ? ' _(forced)_' : ''}`,
  `- mcp image (\`rushdb/mcp-server:${result.mcp_version || '?'}\`): **${mcpChanged ? 'build + deploy' : 'skip'}**${forceMcp ? ' _(forced)_' : ''}`,
  ''
].join('\n')

if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, summary)
console.log(summary)
