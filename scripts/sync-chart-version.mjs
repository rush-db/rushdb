// Keeps the Helm chart in step with the platform release.
//
// Runs as part of `pnpm run version`, i.e. inside `changeset version`, so the
// chart bump travels in the changesets "version packages" PR rather than being
// remembered by hand. Merging that PR changes `charts/**`, which is what
// .github/workflows/helm-publish.yml triggers on — the chart publishes itself.
//
// The chart's own `version` tracks the platform version rather than moving
// independently. Pushing to an OCI registry overwrites an existing chart
// version, so tying it to a number that always advances on release keeps every
// publish addressable. Template-only edits between releases reuse the current
// chart version; bump `platform/core` (and therefore the platform image) if a
// template change needs its own chart release.

import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const read = (file) => readFileSync(join(root, file), 'utf8')

// The platform image is tagged with rushdb-core's version (core and dashboard
// are a `fixed` changesets pair, so this covers both halves of the image).
const version = JSON.parse(read('platform/core/package.json')).version

const EDITS = [
  {
    file: 'charts/rushdb/Chart.yaml',
    what: 'chart version',
    pattern: /^version: .+$/m,
    replacement: `version: ${version}`
  },
  {
    file: 'charts/rushdb/Chart.yaml',
    what: 'chart appVersion',
    pattern: /^appVersion: .+$/m,
    replacement: `appVersion: '${version}'`
  },
  {
    // Anchored on the repository line so the synx and neo4j image tags in the
    // same file are left alone.
    file: 'charts/rushdb/values.yaml',
    what: 'rushdb/platform image tag',
    pattern: /(repository: rushdb\/platform\n\s*tag: ).+$/m,
    replacement: `$1'${version}'`
  }
]

const contents = new Map()

for (const { file, what, pattern, replacement } of EDITS) {
  const before = contents.get(file) ?? read(file)
  if (!pattern.test(before)) {
    // Failing loudly beats silently letting the chart drift again.
    throw new Error(`sync-chart-version: could not locate ${what} in ${file}`)
  }
  contents.set(file, before.replace(pattern, replacement))
}

let changed = false
for (const [file, next] of contents) {
  if (next === read(file)) continue
  writeFileSync(join(root, file), next)
  changed = true
}

console.log(
  changed ?
    `sync-chart-version: Helm chart pinned to ${version}`
  : `sync-chart-version: Helm chart already at ${version}`
)
