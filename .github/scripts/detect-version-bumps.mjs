// Detects whether the 2.x platform version changed in the push that triggered
// this workflow.
//
// The platform image bundles core + dashboard and is tagged with core's
// version, so either half bumping rebuilds it. There are no changesets on the
// 2.x LTS branch: a maintainer bumps both package.json versions (and runs
// `pnpm version:chart` for the Helm chart) in a normal commit.
//
// Versions are read out of git objects (`git show <ref>:<path>`), never off
// disk.
//
// Env:
//   BASE_SHA        state of the branch before this push (github.event.before)
//   HEAD_SHA        commit being released (github.sha); defaults to HEAD
//   FORCE_PLATFORM  'true' → report the platform image as changed regardless

import { execFileSync } from 'node:child_process'
import { appendFileSync } from 'node:fs'

const TRACKED = [
  { key: 'core', path: 'platform/core/package.json' },
  { key: 'dashboard', path: 'platform/dashboard/package.json' }
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
// freshly created ref; fall back to the release commit's first parent.
let base = process.env.BASE_SHA
if (!commitExists(base) || /^0+$/.test(base ?? '')) {
  base = commitExists(`${head}^1`) ? `${head}^1` : null
}

const forcePlatform = process.env.FORCE_PLATFORM === 'true'

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

// core and dashboard ship as one image tagged with core's version. A manual
// 2.x bump must move both together, or the image tag would be reused.
if (result.core_version !== result.dashboard_version) {
  throw new Error(
    `platform/core (${result.core_version || '?'}) and platform/dashboard (${result.dashboard_version || '?'}) ` +
      'versions must match; bump both package.json files together'
  )
}

const platformChanged =
  result.core_changed === 'true' || result.dashboard_changed === 'true' || forcePlatform

result.platform_changed = String(platformChanged)
result.platform_version = result.core_version

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
  `- platform image (\`rushdb/platform:${result.platform_version || '?'}\`): **${platformChanged ? 'build' : 'skip'}**${forcePlatform ? ' _(forced)_' : ''}`,
  ''
].join('\n')

if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, summary)
console.log(summary)
