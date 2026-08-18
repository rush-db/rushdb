<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB Agent Memory Contract

### One memory protocol for OpenClaw, Hermes, and custom agent runtimes.

[![npm](https://img.shields.io/npm/v/%40rushdb%2Fagent-memory-contract)](https://www.npmjs.com/package/@rushdb/agent-memory-contract)
[![license](https://img.shields.io/npm/l/%40rushdb%2Fagent-memory-contract)](#license)

[Website](https://rushdb.com) · [Documentation](https://docs.rushdb.com) · [RushDB Cloud](https://app.rushdb.com)

</div>

---

`@rushdb/agent-memory-contract` is the provider-neutral TypeScript boundary for durable agent memory
in RushDB. It gives runtime adapters a shared event model, deterministic identities, strict scope
filters, semantic recall primitives, and language-neutral conformance resources.

Use it when building a native memory integration for an agent runtime. It keeps stored episodes and
facts compatible across adapters without forcing runtimes to share lifecycle code.

## What it provides

- `AgentMemoryEvent v1`, with `EPISODE` and `MEMORY_FACT` variants
- deterministic SHA-256 `eventId` and `factId` generation
- participant-scope hashing and complete authorization query filters
- idempotent RushDB upserts for episodes and facts
- semantic recall across episode summaries and active facts
- a bounded recent-write cache for recall before embeddings become visible
- formatting that labels recalled memory as untrusted historical context
- a published JSON Schema and cross-language conformance fixture

## Installation

```bash
pnpm add @rushdb/agent-memory-contract
```

## Quick start

```typescript
import {
  RushDBAgentMemory,
  createEpisodeEvent,
  formatRecalledMemories,
  hashScope
} from '@rushdb/agent-memory-contract'

const scope = {
  agentId: 'support-agent',
  profileId: 'default',
  privacyScope: 'private' as const,
  participantScopeHash: hashScope(['discord', 'account-1', 'user-42'], process.env.MEMORY_SCOPE_SALT),
  sandboxEligible: false
}

const memory = new RushDBAgentMemory({
  apiKey: process.env.RUSHDB_API_KEY
})

await memory.ensureIndexes()

await memory.persistEpisode(
  createEpisodeEvent({
    ...scope,
    runtime: 'custom',
    externalSessionId: 'session-42',
    sourceEventId: 'turn-7',
    turnIndex: 7,
    userText: 'Prefer TypeScript for this service.',
    assistantText: 'I will use TypeScript.',
    summary: 'The user prefers TypeScript for this service.',
    conversationKind: 'direct',
    visibility: 'participant',
    trustClass: 'mixed',
    originClass: 'conversation',
    observedAt: new Date().toISOString(),
    provenance: 'custom:completed_turn'
  })
)

const recalled = await memory.recall({
  ...scope,
  query: 'Which language does the user prefer?',
  excludeSessionId: 'session-42',
  limit: 6
})

const context = formatRecalledMemories(recalled)
```

`ensureIndexes()` creates managed embedding indexes for `EPISODE.summary` and
`MEMORY_FACT.text` when they do not already exist.

## Event model

| Event          | RushDB label  | Purpose                                                    |
| -------------- | ------------- | ---------------------------------------------------------- |
| Episode        | `EPISODE`     | A bounded, completed user/assistant interaction            |
| Canonical fact | `MEMORY_FACT` | A durable fact produced by an explicit, trusted write path |

Every event includes the authorization scope:

```text
agentId
profileId
privacyScope
participantScopeHash
sandboxEligible
```

Recall applies all five fields before similarity ranking. Fact recall additionally requires
`active: true`; episode recall can exclude the current external session.

Derive scope only from trusted host metadata. Never accept agent, participant, privacy, or sandbox
scope from model output or prompt text. Use separate RushDB projects when hard tenant isolation is
required.

## Package exports

| Export                                   | Contents                                               |
| ---------------------------------------- | ------------------------------------------------------ |
| `@rushdb/agent-memory-contract`          | TypeScript types, canonical helpers, client, formatter |
| `@rushdb/agent-memory-contract/schema`   | AgentMemoryEvent v1 JSON Schema                        |
| `@rushdb/agent-memory-contract/fixtures` | Deterministic cross-language conformance fixture       |

Other languages should implement the published schema and produce the same IDs as the conformance
fixture.

## Adapter responsibilities

This package is a protocol and client primitive, not a complete runtime adapter. The host
integration remains responsible for:

- proving a conversation is eligible before deriving scope
- capturing only the latest successful, bounded user/assistant pair
- excluding system prompts, tool transcripts, secrets, command output, and local paths
- applying a short fail-open timeout to recall
- writing to a durable outbox before background persistence
- retrying and replaying pending writes without losing idempotency
- deactivating a prior fact when publishing a replacement via `supersedesFactId`
- clearing session state and performing bounded flushes on session end and shutdown

Treat formatted recall as historical data, never as instructions or policy.

## Development

From the RushDB monorepo root:

```bash
pnpm --filter ./packages/agent-memory-contract types:check
pnpm --filter ./packages/agent-memory-contract test
pnpm --filter ./packages/agent-memory-contract pack:check
```

`pack:check` builds the package and verifies that the npm tarball contains the README, runtime
JavaScript, TypeScript declarations, JSON Schema, and conformance fixture.

## License

Apache-2.0
