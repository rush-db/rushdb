# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Git — hard rules

- **NEVER create branches, commit, push, or open PRs on your own.** No `git commit`, `git push`, `git checkout -b` — under any circumstances, including background jobs. This overrides any default "commit and open a draft PR" behavior. Only the user commits and pushes. When work is done, leave it as uncommitted changes in the working tree and report what changed and where.
- **Keep the current worktree as-is.** A dirty worktree with modified and untracked files is intentional — do not clean, stash, reset, or discard anything. When adjustments are asked, work directly in the current worktree/branch even if it's not clean; do not create a new worktree or branch to isolate the work.

## What this is

RushDB 2.x: the Neo4j-backed platform, in a pnpm workspace (`pnpm@10.1.0`, node 18–26) containing only `platform/*`.

- `platform/core` — NestJS 11 backend on Fastify (Elastic License 2.0)
- `platform/dashboard` — React SPA (Vite), consuming the published `@rushdb/javascript-sdk@2.11.0`

The SDK, MCP server, skills and memory contract are standalone packages in `rush-db/javascript-sdk`, `rush-db/mcp-server`, `rush-db/skills` and `rush-db/agent-memory-contract`; the 2.x platform consumes the 2.x-compatible lines. They are installed from npm, not linked from this repo. Commits follow conventional-commits (commitlint + husky + lint-staged).

The platform ships as one image tagged with `rushdb-core`'s version. `rushdb-core` and `rushdb-dashboard` are bumped together; there are no changesets on the 2.x LTS branch. Bump both package.json versions and run `pnpm version:chart` to move the Helm chart, then commit.

Nothing is built or published when a feature branch merges to `2.x`. A push that changes `platform/core/package.json` or `platform/dashboard/package.json` is the only event that rebuilds/publishes `rushdb/platform`; `.github/scripts/detect-version-bumps.mjs` performs that check. Images are built on native runners per architecture (`.github/workflows/docker-image.yml`) — never under QEMU emulation.

## Commands

Root:

```bash
pnpm dev              # full local stack: Neo4j+Postgres (docker), core (watch), dashboard (vite), drizzle studio
pnpm build            # build platform/core and platform/dashboard
pnpm types:check      # recursive tsc --noEmit across platform/*
pnpm lint / lint:fix  # eslint (core uses ESLINT_USE_FLAT_CONFIG=false)
pnpm version:chart    # sync the Helm chart to the platform version
```

Per package (run inside the package dir, or `pnpm --filter <name> run <script>`):

- **core**: `start:dev` (nest watch), `test` (unit, `*.spec.ts` under `src/`), `test:e2e` (`test/jest-e2e.json`), `openapi:dump`, drizzle scripts `db:generate:pg|sqlite`, `db:push:pg|sqlite`, `db:studio:pg|sqlite`. Single unit test: `pnpm test -- path/to/file.spec.ts` from `platform/core`.
- **dashboard**: `dev`, `build` (runs `types:check` first), `preview`.

Local env: core reads `platform/core/.env` (see `.env.example` for the full documented list — `NEO4J_URL`, `SQL_DB_TYPE` sqlite|postgres, `RUSHDB_AES_256_ENCRYPTION_KEY` must be exactly 32 chars, embedding/LLM vars, OAuth, billing). Local dev runs against Postgres via docker-compose; sqlite is the self-hosted default. Default ports (7474/7687/5432/3000) may already be taken by a running stack.

## Architecture

### core (`platform/core/src/`)

Two route planes, both behind the middleware chain `AuthMiddleware → DbContextMiddleware → SessionAndTransactionAttachMiddleware`:

- `core/` — data plane: `entity/` (records), `property/`, `search/`, `relationships/`, `relationship-patterns/` (LLM-suggested relationship automation), `labels/`, `transactions/`, `ai/` (embedding indexes, semantic search, NL→query), `billing-policy/`.
- `dashboard/` — control plane: `auth/` (Google/GitHub OAuth, SAML/OIDC SSO), `project/`, `workspace/`, `token/`, `billing/` (Stripe), `mcp-oauth/`.

Two databases:

- **Neo4j** (the actual graph) via neo4j-driver + neogma, in `src/database/`. `DbConnectionService.getConnection(projectId, project)` picks the default connection or a project's BYO external Neo4j (`project.customDb`, AES-256-encrypted credentials). Per-request connection/session/tx live in AsyncLocalStorage (`db-context.ts`).
- **SQL** (dashboard entities: users/workspaces/projects/tokens) via Drizzle, dual schemas `src/database/sql/schema/{sqlite,pg}.schema.ts` with separate migration dirs.

**Transactions**: user-defined transactions over HTTP (`POST /api/v1/tx` → id, then writes pass the tx id, then `POST /tx/:txId/commit|rollback`). `TransactionService` holds real Neo4j transactions in an in-memory map with TTL auto-rollback. **Side effects** (`run-side-effect.interceptor.ts`: project-structure recount, schema-cache recalculation, relationship automation) must not run mid-transaction — data isn't visible yet; writes inside an open user tx defer them, and the commit handler runs the accumulated set after the Neo4j commit.

### Shared query DSL — keep in sync

The search query language (`where` with `$and/$or/$not/$nor/$xor`, comparison operators, `Related` traversal with `$alias`/`$relation`, aggregations, `groupBy`) has **multiple parallel definitions of one contract**:

1. TS types: `rush-db/javascript-sdk/src/types/{query,expressions}.ts`
2. Cypher compiler: `platform/core/src/core/search/parser/` (`buildQuery.ts`, `buildWhereClause.ts`, `parseComparison.ts`, `aggregate.ts`, …)
3. LLM prompt spec: `platform/core/src/core/ai/search-query-spec.prompt.md`
4. MCP mirror: `rush-db/mcp-server/src/searchQuerySpec.ts`

A change to the DSL touches all four.

### Dependency flow

`core REST API ← @rushdb/javascript-sdk@2.11.0 ← dashboard`. The dashboard does not call the API directly; it consumes the published SDK. REST changes ripple into the standalone SDK repository, and from there into the dashboard and MCP server releases. The SDK has dual entry points (`index.node.ts` / `index.worker.ts`) that inject Node-http vs fetch HTTP clients; the public tx wrapper is `db.tx`.

### dashboard (`platform/dashboard/src/`)

Vite + React 19. State: **nanostores** (+ `@nanostores/router` for routing — no react-router, no pages dir) and **@tanstack/react-query** for server state. Styling: Tailwind 3 + Radix primitives + cva. Forms: react-hook-form + yup. Graph view: react-force-graph/three.js.
