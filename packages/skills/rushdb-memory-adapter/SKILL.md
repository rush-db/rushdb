---
name: rushdb-memory-adapter
description: Design, implement, review, or test native RushDB memory adapters for agent runtimes. Use when adding lifecycle-aware recall and persistence to a framework, mapping prompt/turn/compaction/session hooks, implementing AgentMemoryEvent v1, enforcing participant and sandbox scope, adding durable outbox delivery, or checking an adapter against the OpenClaw and Hermes integration patterns.
---

# RushDB Memory Adapter

Build native adapters that recall before inference, persist bounded completed turns, and remain safe when RushDB is slow or unavailable. Reuse the provider-neutral contract instead of creating a host-specific memory model.

## 1. Inspect the Runtime Contract

Read the exact installed or upstream runtime types and lifecycle source. Do not implement from remembered APIs.

Identify:

- the last hook before inference or prompt construction;
- the successful completed-turn hook;
- explicit memory-write hooks;
- compaction, session switch/end, and shutdown hooks;
- host-provided agent, profile, participant, channel, sandbox, and subagent metadata;
- whether a memory provider is additive or exclusive;
- hook latency and blocking guarantees.

Pin and test against a concrete runtime version.

## 2. Assign Lifecycle Ownership

Map each operation once:

| Operation             | Required behavior                                                |
| --------------------- | ---------------------------------------------------------------- |
| Pre-inference recall  | Short timeout; return no memory on failure                       |
| Completed turn        | Extract only the latest successful user/assistant pair           |
| Explicit memory write | Produce a scoped `MEMORY_FACT` only from a trusted host pathway  |
| Pre-compaction        | Queue the last bounded episode if the host may discard it        |
| Session switch/end    | Clear session caches and flush briefly                           |
| Shutdown              | Attempt a bounded flush; leave pending outbox entries replayable |

Suppress automatic writes from subagents or non-primary contexts unless isolation and ownership are explicitly designed.

## 3. Derive Scope Before Content

Build authorization scope only from host metadata:

```text
agentId
profileId
privacyScope
participantScopeHash
sandboxEligible
```

Hash participant identity with a deployment-specific salt. Fail closed when a conversation cannot be proven eligible. Never accept scope fields from model output or prompt text.

Use separate RushDB projects for hard tenant isolation.

## 4. Reuse AgentMemoryEvent v1

Use `@rushdb/agent-memory-contract` in TypeScript or implement its published JSON Schema and conformance fixture exactly in another language.

- Compute deterministic `eventId` and `factId` from canonical JSON.
- Persist `EPISODE` with `mergeBy: ['eventId']`.
- Persist `MEMORY_FACT` with `mergeBy: ['factId']`.
- Recall active facts only.
- Preserve fact history with `supersedesFactId` and prior-version deactivation.

Do not fork the schema for one runtime. Propose a versioned contract change when a host needs new provider-neutral fields.

## 5. Implement Recall as an Authorization Query

Create managed embedding indexes for `EPISODE.summary` and `MEMORY_FACT.text`.

For every vector search:

1. Apply all scope fields in structured `where`.
2. Add `active: true` for facts.
3. Exclude the current session when the host should not recall its own turn history.
4. Bound query length and result count.
5. Merge episode, fact, and recent-write results by deterministic ID.
6. Format results as untrusted quoted context, never instructions.

Use a tight fail-open timeout on the inference path. Prefetch asynchronously when the runtime offers an earlier hook.

## 6. Make Writes Durable and Non-Blocking

Append the canonical event to a profile-local durable outbox before acknowledging a background write. Then enqueue it for a bounded worker pool.

The worker must:

- retry transient failures with bounded backoff;
- replay pending files after restart;
- delete an outbox entry only after idempotent remote persistence succeeds;
- prevent duplicate in-process queue entries;
- bound memory, concurrency, and shutdown time.

Keep a bounded recent-write cache so recall works before managed embeddings become visible. Never keep a RushDB transaction open across an LLM turn.

## 7. Bound the Capture Surface

Persist only required, normalized user and assistant text plus a bounded summary. Never automatically persist:

- system or developer prompts;
- full message arrays;
- tool calls or tool results;
- secrets, environment values, command output, or local paths;
- unrelated channel or group history.

Record provenance, origin, trust, visibility, and timestamps explicitly.

## 8. Validate Before Release

Read [lifecycle-conformance.md](references/lifecycle-conformance.md) and execute the applicable test matrix.

At minimum verify:

- cross-language deterministic IDs against the shared fixture;
- exact scope prefilters on every canonical read;
- sandbox/group/subagent rejection;
- latest-pair extraction and transcript exclusions;
- fail-open recall timeout;
- durable retry/restart replay;
- recent-write recall before embedding readiness;
- bounded session-end and shutdown behavior;
- compatibility with the pinned upstream runtime.

Document which capabilities are unsupported instead of silently approximating them.
