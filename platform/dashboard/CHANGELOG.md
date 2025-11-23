# rushdb-dashboard

## 1.19.0

### Minor Changes

- 865ba18: Add merge/upsert for bulk importing or single record creation

### Patch Changes

- Updated dependencies [865ba18]
  - @rushdb/javascript-sdk@1.19.0

## 1.18.0

### Minor Changes

- 488c1d1: createMany & importJson separation plus minor export and import improvements

### Patch Changes

- Updated dependencies [488c1d1]
  - @rushdb/javascript-sdk@1.18.0

## 1.17.0

### Minor Changes

- 71ca63f: Add MCP Server package

### Patch Changes

- Updated dependencies [71ca63f]
  - @rushdb/javascript-sdk@1.17.0

## 1.16.0

### Minor Changes

- fb311f9: Add timeBucket aggregations

### Patch Changes

- Updated dependencies [fb311f9]
  - @rushdb/javascript-sdk@1.16.0

## 1.15.3

### Patch Changes

- 32bd9c1: Increased requests limits for Throttler
- Updated dependencies [32bd9c1]
  - @rushdb/javascript-sdk@1.15.3

## 1.15.2

### Patch Changes

- 9b25c07: Make int txn optionally closed after commit
- Updated dependencies [9b25c07]
  - @rushdb/javascript-sdk@1.15.2

## 1.15.1

### Patch Changes

- 954ab72: Fix ts bug in aggregations
- Updated dependencies [954ab72]
  - @rushdb/javascript-sdk@1.15.1

## 1.15.0

### Minor Changes

- 7f19708: ## Summary
  Adds first-class grouping support to the Search API (`groupBy`) across core, JavaScript SDK, dashboard, website, and docs. Also standardizes terminology (`uniq` -> `unique`), refines aggregation semantics, and updates documentation with a dedicated grouping concept page.

  ***

  ## ✨ New Feature: `groupBy` Clause

  You can now pivot / summarize search results by one or more keys. Keys reference an alias + property (root alias is implicitly `$record`).

  Example (JS SDK):

  ```ts
  const dealsByStage = await db.records.find({
    labels: ['HS_DEAL'],
    aggregate: {
      count: { fn: 'count', alias: '$record' },
      avgAmount: { fn: 'avg', field: 'amount', alias: '$record' }
    },
    groupBy: ['$record.dealstage'],
    orderBy: { count: 'desc' }
  })
  // → rows like: [{ dealstage: 'prospecting', count: 120, avgAmount: 3400 }, ...]
  ```

  Key capabilities:

  - Multiple grouping keys: `groupBy: ['$record.category', '$record.active']`
  - Group by related aliases (declare alias in `where` traversal first)
  - Works with all existing aggregation functions (count, sum, avg, min, max, collect, similarity, etc.)
  - Ordering applies to aggregated rows when `groupBy` is present
  - Requires at least one aggregation entry to take effect

  Result shape when using `groupBy`: each row contains only the grouping fields plus aggregated fields (raw record bodies are not returned unless you also aggregate them via `collect`).

  ***

  ## 🔄 Aggregation & Semantics Updates

  - `collect` results are unique by default. Set `unique: false` to retain duplicates.
  - Aggregation entries now consistently use the `unique` flag (replacing legacy `uniq`).
  - Distinct handling for grouped queries unified under the `unique` option.
  - Improved Cypher generation: clearer alias usage and property quoting; vector similarity function formatting tightened.
  - Added internal `PROPERTY_WILDCARD_PROJECTION` support (enables future selective projections) – not yet a public API, but impacts generated queries.

  ***

  ## 💥 Breaking Changes

  | Area                                | Change                                              | Action Required                                                                                                          |
  | ----------------------------------- | --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
  | Schema field definitions            | `uniq` key renamed to `unique`                      | Rename all occurrences (`{ uniq: true }` → `{ unique: true }`).                                                          |
  | Aggregation definitions             | Aggregator option `uniq` renamed to `unique`        | Update custom aggregation objects (`uniq: false` → `unique: false`).                                                     |
  | Result shape (when using `groupBy`) | Raw record objects no longer returned automatically | If you previously expected full records, add a `collect` aggregation (e.g. `rows: { fn: 'collect', alias: '$record' }`). |
  | Default uniqueness for `collect`    | Now unique by default                               | Add `unique: false` if you require duplicates.                                                                           |
  | Internal alias constant             | `DEFAULT_RECORD_ALIAS` → `ROOT_RECORD_ALIAS`        | Only relevant if you referenced internal constants (avoid relying on these).                                             |

  If any code or saved JSON queries still send `uniq`, they will now fail unless a compatibility shim exists (none added in this release). Treat this as a required migration.

  ***

  ## 🛠 Migration Guide

  1. Rename all schema property options:
     - Before: `email: { type: 'string', uniq: true }`
     - After: `email: { type: 'string', unique: true }`
  2. Update aggregation specs:
     - Before: `names: { fn: 'collect', field: 'name', alias: '$user', uniq: true }`
     - After: `names: { fn: 'collect', field: 'name', alias: '$user' }` (omit `unique` if true)
  3. Reintroduce duplicate collection (if needed): add `unique: false`.
  4. When adopting `groupBy`, ensure at least one aggregation is defined; queries with only `groupBy` are invalid.
  5. Adjust consumer code to handle aggregated row shape instead of full record instances.
  6. For hierarchical drill‑downs: group at the parent level; use nested `collect` for children instead of adding child keys to `groupBy`.

  ### Example Migration (JS)

  ```diff
   aggregate: {
  -  employeeNames: { fn: 'collect', field: 'name', alias: '$employee', uniq: true },
  +  employeeNames: { fn: 'collect', field: 'name', alias: '$employee' },
   }
  ```

  ### Adding Grouping

  ```ts
  const deptProjects = await db.records.find({
    labels: ['DEPARTMENT'],
    where: { PROJECT: { $alias: '$project' } },
    aggregate: {
      projectCount: { fn: 'count', alias: '$project' },
      projects: { fn: 'collect', field: 'name', alias: '$project', unique: true }
    },
    groupBy: ['$record.name'],
    orderBy: { projectCount: 'desc' }
  })
  ```

  ***

  ## 📘 Documentation

  - Added dedicated concept page: `concepts/search/group-by` centralizing all grouping patterns (multi-key, alias-based, nested, uniqueness nuances, limitations).
  - Updated Python, REST, and TypeScript SDK "Get Records" guides with concise grouping sections linking to the concept page.
  - Refactored Aggregations doc to avoid duplication and point to new grouping guide.
  - Standardized examples to use `unique` terminology.

  ***

  ## 🧪 Tests & Internal Refactors

  - Extended aggregate & query builder tests to cover `groupBy` permutations (single key, multi-key, alias grouping, collect uniqueness flags).
  - Parser adjustments for: property quoting, alias resolution, vector similarity formatting, optional matches, and root alias constant rename.
  - Introduced `AggregateContext` enhancements to track grouping state.

  ***

  ## ⚠️ Edge Cases & Notes

  - An empty `groupBy` array is ignored; supply at least one key.
  - Supplying a group key for a property that does not exist yields rows with `null` for that column (consistent with underlying graph behavior) – validate upstream if needed.
  - Ordering by an aggregation that isn't defined will be rejected; always define the aggregate you sort by.
  - To sort by a group key, just reference it in `orderBy` using the property name (without alias prefix) after grouping.

  ***

  ## ✅ Quick Checklist

  | Task                                                                | Done? |
  | ------------------------------------------------------------------- | ----- |
  | Renamed all `uniq` → `unique` in schema & aggregations              |       |
  | Reviewed any `collect` aggregations for unintended de-duplication   |       |
  | Added `unique: false` where duplicates are required                 |       |
  | Updated UI / API consumers for aggregated row shape under `groupBy` |       |
  | Added `collect` fields if raw record snapshots are still needed     |       |
  | Added / validated ordering under grouped queries                    |       |

  ***

  ## Feedback

  Please report any unexpected behavior with grouped queries (especially multi-key or alias-based grouping) so we can refine edge case handling in upcoming releases.

  ***

  ## TL;DR

  Use `groupBy` + `aggregate` to pivot results; rename `uniq` → `unique`; `collect` is now unique by default; aggregated queries return row sets, not raw records.

### Patch Changes

- Updated dependencies [7f19708]
  - @rushdb/javascript-sdk@1.15.0

## 1.14.2

### Patch Changes

- 63823d0: Optimize DbContextMiddleware
- Updated dependencies [63823d0]
  - @rushdb/javascript-sdk@1.14.2

## 1.14.1

### Patch Changes

- 9cafe44: Transactions management improvements
- Updated dependencies [9cafe44]
  - @rushdb/javascript-sdk@1.14.1

## 1.14.0

### Minor Changes

- 7cd984c: Added `records.importCsv` method with configurable CSV `parseConfig` and extended import docs.

### Patch Changes

- Updated dependencies [7cd984c]
  - @rushdb/javascript-sdk@1.14.0

## 1.13.2

### Patch Changes

- ed6063d: Fix missmatching transaction in import service
- Updated dependencies [ed6063d]
  - @rushdb/javascript-sdk@1.13.2

## 1.13.1

### Patch Changes

- 2e89930: Safely handling properties with spaces in name
- Updated dependencies [2e89930]
  - @rushdb/javascript-sdk@1.13.1

## 1.13.0

### Minor Changes

- 2b76c22: Add raw cypher query support

### Patch Changes

- Updated dependencies [2b76c22]
  - @rushdb/javascript-sdk@1.13.0

## 1.12.1

### Patch Changes

- d55ae51: Escape stringified JSON characters fix
- Updated dependencies [d55ae51]
  - @rushdb/javascript-sdk@1.12.1

## 1.12.0

### Minor Changes

- bd84662: Create many relationships by key(s)

### Patch Changes

- Updated dependencies [bd84662]
  - @rushdb/javascript-sdk@1.12.0

## 1.11.1

### Patch Changes

- 00d100b: Fix leaking transactions pool
- Updated dependencies [00d100b]
  - @rushdb/javascript-sdk@1.11.1

## 1.11.0

### Minor Changes

- 43109e7: Fix misconfigured identity matching for external databases

### Patch Changes

- Updated dependencies [43109e7]
  - @rushdb/javascript-sdk@1.11.0

## 1.10.1

### Patch Changes

- 3bc1768: Fix processing boolean response at createMany method
- Updated dependencies [3bc1768]
  - @rushdb/javascript-sdk@1.10.1

## 1.10.0

### Minor Changes

- 83b7ee4: Top level aggregations now works in a predicatable manner

### Patch Changes

- Updated dependencies [83b7ee4]
  - @rushdb/javascript-sdk@1.10.0

## 1.9.1

### Patch Changes

- 742bee5: fix: changed managedDb property type
- Updated dependencies [742bee5]
  - @rushdb/javascript-sdk@1.9.1

## 1.9.0

### Minor Changes

- 5e7ea2c: Feat: added non-encrypted prefix for token to setup server settings in sdk

### Patch Changes

- Updated dependencies [5e7ea2c]
  - @rushdb/javascript-sdk@1.9.0

## 1.8.1

### Patch Changes

- c239ad9: Fix: Allowed non-email logins for self-hosted instances
- Updated dependencies [c239ad9]
  - @rushdb/javascript-sdk@1.8.1

## 1.8.0

### Minor Changes

- 3312b28: Implemented ISR (Incremental Static Regeneration) for website, update dependencies, update docs, fix 3D view issue for records without a relationship (blank canvas)

### Patch Changes

- Updated dependencies [3312b28]
  - @rushdb/javascript-sdk@1.8.0

## 1.7.1

### Patch Changes

- b68c596: Fix error on an attempt of mapping result in createMany method
- Updated dependencies [b68c596]
  - @rushdb/javascript-sdk@1.7.1

## 1.7.0

### Minor Changes

- f9a386d: Implement $exists and $type operators

### Patch Changes

- Updated dependencies [f9a386d]
  - @rushdb/javascript-sdk@1.7.0

## 1.6.0

### Minor Changes

- aae7461: Update aggregation behavior: all own properties of top-level record now included by default

### Patch Changes

- Updated dependencies [aae7461]
  - @rushdb/javascript-sdk@1.6.0

## 1.5.1

### Patch Changes

- 090a6ab: Fix empty projectId for customdb connection
- Updated dependencies [090a6ab]
  - @rushdb/javascript-sdk@1.5.1

## 1.5.0

### Minor Changes

- a920ddf: Add cypher query preview

### Patch Changes

- Updated dependencies [a920ddf]
  - @rushdb/javascript-sdk@1.5.0

## 1.4.0

### Minor Changes

- 3b52549: New SDK Architecture and minor bug fixes

### Patch Changes

- Updated dependencies [3b52549]
  - @rushdb/javascript-sdk@1.4.0

## 1.3.2

### Patch Changes

- 5ec730f: Add server logs and update deployment tutorial
- Updated dependencies [5ec730f]
  - @rushdb/javascript-sdk@1.3.2

## 1.3.1

### Patch Changes

- 43d355b: Fix multiple records deletion error
- Updated dependencies [43d355b]
  - @rushdb/javascript-sdk@1.3.1

## 1.3.0

### Minor Changes

- 7f1bf8d: Add onboarding tour

### Patch Changes

- Updated dependencies [7f1bf8d]
  - @rushdb/javascript-sdk@1.3.0

## 1.2.2

### Patch Changes

- 1720499: fix: simplified delete project flow, fixed orphan props deletion method
- Updated dependencies [1720499]
  - @rushdb/javascript-sdk@1.2.2

## 1.2.1

### Patch Changes

- 7a4c193: Fix delete records by ids method
- Updated dependencies [7a4c193]
  - @rushdb/javascript-sdk@1.2.1

## 1.2.0

### Minor Changes

- 99ac372: Fix label api error, add auth header verification for sdk, values now searchable with SearchQuery, update docs, user can now leave workspace

### Patch Changes

- Updated dependencies [99ac372]
  - @rushdb/javascript-sdk@1.2.0

## 1.1.0

### Minor Changes

- 1ba17a1: Implemented ordering by aggregated field

### Patch Changes

- Updated dependencies [1ba17a1]
  - @rushdb/javascript-sdk@1.1.0

## 1.0.3

### Patch Changes

- 3d3da80: Remove the billing tab for developers the workspace layout
- Updated dependencies [3d3da80]
  - @rushdb/javascript-sdk@1.0.3

## 1.0.2

### Patch Changes

- da7874f: fix: improved invitation accept flow, normalized checkers for invitee emails
- Updated dependencies [da7874f]
  - @rushdb/javascript-sdk@1.0.2

## 1.0.1

### Patch Changes

- 2f1f084: Add local neo4j plugins and fix create method payload
- Updated dependencies [2f1f084]
  - @rushdb/javascript-sdk@1.0.1

## 1.0.0

### Major Changes

- 9d10588: # RushDB 1.0 🚀

  We're thrilled to announce RushDB's first major release with significant new capabilities:

  ## Key Features

  - **Vector Search**: Added comprehensive vector search functionality with similarity aggregates and query builder support
  - **Member Management**: Implemented complete workspace membership system with invitations, user access controls, and per-user project assignments
  - **Remote Database Connectivity**: Added support for remote Neo4j/Aura connections, expanding deployment options
  - **Authentication Enhancements**: Added Google OAuth support and improved user authorization flows
  - **Documentation Overhaul**: Completely reworked documentation with new tutorials and clearer guides

  ## Security & Administration

  - Improved user access control with revoke-access endpoint and normalized user deletion
  - Enhanced `@roles` guard implementation for better permission handling
  - Added workspace billing accuracy improvements
  - Implemented recompute-access-list functionality

  This major release represents a significant milestone in RushDB's development with enterprise-ready features and improved developer experience.

### Patch Changes

- Updated dependencies [9d10588]
  - @rushdb/javascript-sdk@1.0.0

## 0.17.1

### Patch Changes

- f0a395c: SEO Optimizations
- Updated dependencies [f0a395c]
  - @rushdb/javascript-sdk@0.17.1

## 0.17.0

### Minor Changes

- 1cdfbf7: Implemented RawApi mode for the dashboard and minor fixes

### Patch Changes

- 29940a0: Fix lock file
- Updated dependencies [29940a0]
- Updated dependencies [1cdfbf7]
  - @rushdb/javascript-sdk@0.17.0

## 0.16.0

### Minor Changes

- 57a566b: Update LP & Relationships UI and Query

### Patch Changes

- Updated dependencies [57a566b]
  - @rushdb/javascript-sdk@0.16.0

## 0.15.0

### Minor Changes

- 6aa0dca: Update docs
- bcdf10d: Add llms.txt and llms-full.txt to docs generation

### Patch Changes

- Updated dependencies [6aa0dca]
- Updated dependencies [bcdf10d]
  - @rushdb/javascript-sdk@0.15.0

## 0.14.1

### Patch Changes

- 7bfd0aa: Make label field required in import modal in dashboard
- Updated dependencies [7bfd0aa]
  - @rushdb/javascript-sdk@0.14.1

## 0.14.0

### Minor Changes

- c7936fa: Update billing and limits

### Patch Changes

- Updated dependencies [c7936fa]
  - @rushdb/javascript-sdk@0.14.0

## 0.13.3

### Patch Changes

- 9919a34: chore: website sitemap.xml updated
- Updated dependencies [9919a34]
  - @rushdb/javascript-sdk@0.13.3

## 0.13.2

### Patch Changes

- 646101a: Update website blog
- Updated dependencies [646101a]
  - @rushdb/javascript-sdk@0.13.2

## 0.13.1

### Patch Changes

- 12d76f6: Update website
- Updated dependencies [12d76f6]
  - @rushdb/javascript-sdk@0.13.1

## 0.13.0

### Minor Changes

- d045368: Introduce SDK options

### Patch Changes

- Updated dependencies [d045368]
  - @rushdb/javascript-sdk@0.13.0

## 0.12.2

### Patch Changes

- 2019434: Improved local development setup
- Updated dependencies [2019434]
  - @rushdb/javascript-sdk@0.12.2

## 0.12.1

### Patch Changes

- fe5c5f4: Fix cypher clauses order in querying property values
- Updated dependencies [fe5c5f4]
  - @rushdb/javascript-sdk@0.12.1

## 0.12.0

### Minor Changes

- f703e9c: Update query for fetching property values and minor fixes & cleanups

### Patch Changes

- Updated dependencies [f703e9c]
  - @rushdb/javascript-sdk@0.12.0

## 0.11.6

### Patch Changes

- 633047a: Fix missing return in get transaction method
- b4599fc: Fix missing label criteria in delete request
- 837f17a: Extended logger for SDK
- Updated dependencies [633047a]
- Updated dependencies [b4599fc]
- Updated dependencies [837f17a]
  - @rushdb/javascript-sdk@0.11.6

## 0.11.5

### Patch Changes

- c373116: Minor docs update and temporary disabled gh auth
- Updated dependencies [c373116]
  - @rushdb/javascript-sdk@0.11.5

## 0.11.4

### Patch Changes

- 08b5d22: Fix Dockerfile pnpm version
- Updated dependencies [08b5d22]
  - @rushdb/javascript-sdk@0.11.4

## 0.11.3

### Patch Changes

- 5a37f89: Version bump to fix pnpm workflow version
- Updated dependencies [5a37f89]
  - @rushdb/javascript-sdk@0.11.3

## 0.11.2

### Patch Changes

- c7bd389: Version bump to fix pnpm workflow version
- Updated dependencies [c7bd389]
  - @rushdb/javascript-sdk@0.11.2

## 0.11.1

### Patch Changes

- 4b13961: Fix github ouath login

  Make GraphView responsive

  Fix Password recovery flow

- Updated dependencies [4b13961]
  - @rushdb/javascript-sdk@0.11.1

## 0.11.0

### Minor Changes

- 85038ac: Update readme, website, add python package and examples

### Patch Changes

- Updated dependencies [85038ac]
  - @rushdb/javascript-sdk@0.11.0

## 0.10.2

### Patch Changes

- 476acdf: Update examples link
- Updated dependencies [476acdf]
  - @rushdb/javascript-sdk@0.10.2

## 0.10.1

### Patch Changes

- 0442ddc: Make SDK Instance method public
- Updated dependencies [0442ddc]
  - @rushdb/javascript-sdk@0.10.1

## 0.10.0

### Minor Changes

- f159dfa: Improve type inference for SDK

### Patch Changes

- Updated dependencies [f159dfa]
  - @rushdb/javascript-sdk@0.10.0

## 0.9.5

### Patch Changes

- 7ea9151: Update bundling for SDK and minor typo fixes
- fc33a7e: Fix types import and export issues
- Updated dependencies [7ea9151]
- Updated dependencies [fc33a7e]
  - @rushdb/javascript-sdk@0.9.5

## 0.9.4

### Patch Changes

- 09e2d57: Update docs, minor fixes and cleanups
- Updated dependencies [09e2d57]
  - @rushdb/javascript-sdk@0.9.4

## 0.9.3

### Patch Changes

- 22ce867: billing setup update for dev/prod envs
- Updated dependencies [22ce867]
  - @rushdb/javascript-sdk@0.9.3

## 0.9.2

### Patch Changes

- eafe1b5: updated secret credentials
- Updated dependencies [eafe1b5]
  - @rushdb/javascript-sdk@0.9.2

## 0.9.1

### Patch Changes

- b055539: Dashboard/website: Introduced start plan, updated dashboard && website layouts
- Updated dependencies [b055539]
  - @rushdb/javascript-sdk@0.9.1

## 0.9.0

### Minor Changes

- 6cc7392: Release 0.9.0 version with updated billing

### Patch Changes

- Updated dependencies [6cc7392]
  - @rushdb/javascript-sdk@0.9.0

## 0.8.2

### Patch Changes

- Updated dependencies [6e900d7]
  - @rushdb/javascript-sdk@0.8.2

## 0.8.1

### Patch Changes

- 3d65528: Core/Dashboard/Website now accept billing data from internal billing service instead of hardcoded values

## 0.8.0

### Minor Changes

- 49e3153: Dependencies update & landing page rework

### Patch Changes

- Updated dependencies [49e3153]
  - @rushdb/javascript-sdk@0.8.0

## 0.7.2

### Patch Changes

- c32b9bf: Aggregations minor fixes
- Updated dependencies [c32b9bf]
  - @rushdb/javascript-sdk@0.7.2

## 0.7.1

### Patch Changes

- 0551051: Restore original skip & limit behaviour for nested search
- Updated dependencies [0551051]
  - @rushdb/javascript-sdk@0.7.1

## 0.7.0

### Minor Changes

- 5470782: Fix result cut-off for nested queries and url parsing in node.js env at sdk's networking

### Patch Changes

- Updated dependencies [5470782]
  - @rushdb/javascript-sdk@0.7.0

## 0.6.0

### Minor Changes

- 6ec52f2: Add simple backup module, cli command and minor fixes

### Patch Changes

- Updated dependencies [6ec52f2]
  - @rushdb/javascript-sdk@0.6.0

## 0.5.0

### Minor Changes

- 3112626: Implemented logger feature for sdk

### Patch Changes

- Updated dependencies [3112626]
  - @rushdb/javascript-sdk@0.5.0

## 0.4.1

### Patch Changes

- 070d094: Minor cloud update
- Updated dependencies [070d094]
  - @rushdb/javascript-sdk@0.4.1

## 0.4.0

### Minor Changes

- 0e04a46: Version bump

### Patch Changes

- 4f83de0: Minor fixes and updates
- defeaf0: Minor fixes and updates
- Updated dependencies [0e04a46]
- Updated dependencies [4f83de0]
- Updated dependencies [defeaf0]
  - @rushdb/javascript-sdk@0.4.0

## 0.3.1

### Patch Changes

- d3b73ac: Minor fixes
- Updated dependencies [d3b73ac]
  - @rushdb/javascript-sdk@0.3.1

## 0.3.0

### Minor Changes

- 70d108d: CLI commands & datetime helpers in the Javascript SDK

### Patch Changes

- Updated dependencies [70d108d]
  - @rushdb/javascript-sdk@0.3.0

## 0.2.10

### Patch Changes

- 89f2783: Update readme and license
- Updated dependencies [89f2783]
  - @rushdb/javascript-sdk@0.2.10

## 0.2.9

### Patch Changes

- 9580ede: Update infrastructure and minor fixes
- Updated dependencies [9580ede]
  - @rushdb/javascript-sdk@0.2.9

## 0.2.8

### Patch Changes

- 1cf62ae: Minor fixes and cleanups
- Updated dependencies [1cf62ae]
  - @rushdb/javascript-sdk@0.2.8

## 0.2.7

### Patch Changes

- 147cc2b: Build docker image only on version bump
- Updated dependencies [147cc2b]
  - @rushdb/javascript-sdk@0.2.7

## 0.2.6

### Patch Changes

- 89852a4: Update docker image release
- Updated dependencies [89852a4]
  - @rushdb/javascript-sdk@0.2.6

## 0.2.5

### Patch Changes

- cd0be33: Release flow improvements
- Updated dependencies [cd0be33]
  - @rushdb/javascript-sdk@0.2.5

## 0.2.4

### Patch Changes

- 3741a7e: Release workflow change
- Updated dependencies [3741a7e]
  - @rushdb/javascript-sdk@0.2.4

## 0.2.3

### Patch Changes

- c7f1eb9: Update workflow
- Updated dependencies [c7f1eb9]
  - @rushdb/javascript-sdk@0.2.3

## 0.2.2

### Patch Changes

- de0fb17: Update release workflow
- Updated dependencies [de0fb17]
  - @rushdb/javascript-sdk@0.2.2

## 0.2.1

### Patch Changes

- f934b0f: Bump version
- Updated dependencies [f934b0f]
  - @rushdb/javascript-sdk@0.2.1

## 0.2.0

### Minor Changes

- cb1e4be: Update readme, cleanups, build public image

### Patch Changes

- Updated dependencies [cb1e4be]
  - @rushdb/javascript-sdk@0.2.0
