# rushdb-core

## 1.11.0

### Minor Changes

- 43109e7: Fix misconfigured identity matching for external databases

## 1.10.1

### Patch Changes

- 3bc1768: Fix processing boolean response at createMany method

## 1.10.0

### Minor Changes

- 83b7ee4: Top level aggregations now works in a predicatable manner

## 1.9.1

### Patch Changes

- 742bee5: fix: changed managedDb property type

## 1.9.0

### Minor Changes

- 5e7ea2c: Feat: added non-encrypted prefix for token to setup server settings in sdk

## 1.8.1

### Patch Changes

- c239ad9: Fix: Allowed non-email logins for self-hosted instances

## 1.8.0

### Minor Changes

- 3312b28: Implemented ISR (Incremental Static Regeneration) for website, update dependencies, update docs, fix 3D view issue for records without a relationship (blank canvas)

## 1.7.1

### Patch Changes

- b68c596: Fix error on an attempt of mapping result in createMany method

## 1.7.0

### Minor Changes

- f9a386d: Implement $exists and $type operators

## 1.6.0

### Minor Changes

- aae7461: Update aggregation behavior: all own properties of top-level record now included by default

## 1.5.1

### Patch Changes

- 090a6ab: Fix empty projectId for customdb connection

## 1.5.0

### Minor Changes

- a920ddf: Add cypher query preview

## 1.4.0

### Minor Changes

- 3b52549: New SDK Architecture and minor bug fixes

## 1.3.2

### Patch Changes

- 5ec730f: Add server logs and update deployment tutorial

## 1.3.1

### Patch Changes

- 43d355b: Fix multiple records deletion error

## 1.3.0

### Minor Changes

- 7f1bf8d: Add onboarding tour

## 1.2.2

### Patch Changes

- 1720499: fix: simplified delete project flow, fixed orphan props deletion method

## 1.2.1

### Patch Changes

- 7a4c193: Fix delete records by ids method

## 1.2.0

### Minor Changes

- 99ac372: Fix label api error, add auth header verification for sdk, values now searchable with SearchQuery, update docs, user can now leave workspace

## 1.1.0

### Minor Changes

- 1ba17a1: Implemented ordering by aggregated field

## 1.0.3

### Patch Changes

- 3d3da80: Remove the billing tab for developers the workspace layout

## 1.0.2

### Patch Changes

- da7874f: fix: improved invitation accept flow, normalized checkers for invitee emails

## 1.0.1

### Patch Changes

- 2f1f084: Add local neo4j plugins and fix create method payload

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

## 0.17.1

### Patch Changes

- f0a395c: SEO Optimizations

## 0.17.0

### Minor Changes

- 1cdfbf7: Implemented RawApi mode for the dashboard and minor fixes

### Patch Changes

- 29940a0: Fix lock file

## 0.16.0

### Minor Changes

- 57a566b: Update LP & Relationships UI and Query

## 0.15.0

### Minor Changes

- 6aa0dca: Update docs
- bcdf10d: Add llms.txt and llms-full.txt to docs generation

## 0.14.1

### Patch Changes

- 7bfd0aa: Make label field required in import modal in dashboard

## 0.14.0

### Minor Changes

- c7936fa: Update billing and limits

## 0.13.3

### Patch Changes

- 9919a34: chore: website sitemap.xml updated

## 0.13.2

### Patch Changes

- 646101a: Update website blog

## 0.13.1

### Patch Changes

- 12d76f6: Update website

## 0.13.0

### Minor Changes

- d045368: Introduce SDK options

## 0.12.2

### Patch Changes

- 2019434: Improved local development setup

## 0.12.1

### Patch Changes

- fe5c5f4: Fix cypher clauses order in querying property values

## 0.12.0

### Minor Changes

- f703e9c: Update query for fetching property values and minor fixes & cleanups

## 0.11.6

### Patch Changes

- 633047a: Fix missing return in get transaction method
- b4599fc: Fix missing label criteria in delete request
- 837f17a: Extended logger for SDK

## 0.11.5

### Patch Changes

- c373116: Minor docs update and temporary disabled gh auth

## 0.11.4

### Patch Changes

- 08b5d22: Fix Dockerfile pnpm version

## 0.11.3

### Patch Changes

- 5a37f89: Version bump to fix pnpm workflow version

## 0.11.2

### Patch Changes

- c7bd389: Version bump to fix pnpm workflow version

## 0.11.1

### Patch Changes

- 4b13961: Fix github ouath login

  Make GraphView responsive

  Fix Password recovery flow

## 0.11.0

### Minor Changes

- 85038ac: Update readme, website, add python package and examples

## 0.10.2

### Patch Changes

- 476acdf: Update examples link

## 0.10.1

### Patch Changes

- 0442ddc: Make SDK Instance method public

## 0.10.0

### Minor Changes

- f159dfa: Improve type inference for SDK

## 0.9.5

### Patch Changes

- 7ea9151: Update bundling for SDK and minor typo fixes
- fc33a7e: Fix types import and export issues

## 0.9.4

### Patch Changes

- 09e2d57: Update docs, minor fixes and cleanups

## 0.9.3

### Patch Changes

- 22ce867: billing setup update for dev/prod envs

## 0.9.2

### Patch Changes

- eafe1b5: updated secret credentials

## 0.9.1

### Patch Changes

- b055539: Dashboard/website: Introduced start plan, updated dashboard && website layouts

## 0.9.0

### Minor Changes

- 6cc7392: Release 0.9.0 version with updated billing

## 0.8.1

### Patch Changes

- 3d65528: Core/Dashboard/Website now accept billing data from internal billing service instead of hardcoded values

## 0.8.0

### Minor Changes

- 49e3153: Dependencies update & landing page rework

## 0.7.2

### Patch Changes

- c32b9bf: Aggregations minor fixes

## 0.7.1

### Patch Changes

- 0551051: Restore original skip & limit behaviour for nested search

## 0.7.0

### Minor Changes

- 5470782: Fix result cut-off for nested queries and url parsing in node.js env at sdk's networking

## 0.6.0

### Minor Changes

- 6ec52f2: Add simple backup module, cli command and minor fixes

## 0.5.0

### Minor Changes

- 3112626: Implemented logger feature for sdk

## 0.4.1

### Patch Changes

- 070d094: Minor cloud update

## 0.4.0

### Minor Changes

- 0e04a46: Version bump

### Patch Changes

- 4f83de0: Minor fixes and updates
- defeaf0: Minor fixes and updates

## 0.3.1

### Patch Changes

- d3b73ac: Minor fixes

## 0.3.0

### Minor Changes

- 70d108d: CLI commands & datetime helpers in the Javascript SDK

## 0.2.10

### Patch Changes

- 89f2783: Update readme and license

## 0.2.9

### Patch Changes

- 9580ede: Update infrastructure and minor fixes

## 0.2.8

### Patch Changes

- 1cf62ae: Minor fixes and cleanups

## 0.2.7

### Patch Changes

- 147cc2b: Build docker image only on version bump

## 0.2.6

### Patch Changes

- 89852a4: Update docker image release

## 0.2.5

### Patch Changes

- cd0be33: Release flow improvements

## 0.2.4

### Patch Changes

- 3741a7e: Release workflow change

## 0.2.3

### Patch Changes

- c7f1eb9: Update workflow

## 0.2.2

### Patch Changes

- de0fb17: Update release workflow

## 0.2.1

### Patch Changes

- f934b0f: Bump version

## 0.2.0

### Minor Changes

- cb1e4be: Update readme, cleanups, build public image
