# Memory Adapter Lifecycle Conformance

## Table of Contents

1. Contract tests
2. Authorization tests
3. Capture tests
4. Recall tests
5. Delivery tests
6. Lifecycle tests
7. Compatibility checks

## 1. Contract Tests

- Load the shared AgentMemoryEvent v1 conformance fixture.
- Produce identical episode and fact IDs in every supported language.
- Reject missing or blank `agentId`, `profileId`, and `participantScopeHash`.
- Validate events against the published JSON Schema.
- Verify repeated delivery upserts one logical record.
- Verify fact replacement deactivates the old fact and links the new `supersedesFactId`.

## 2. Authorization Tests

Exercise distinct agents, profiles, participants, privacy scopes, and sandbox eligibility values. A query for one scope must never return another scope's records.

Test rejection or no-op behavior for every host context that cannot supply trusted direct/private authorization, including:

- group and channel conversations;
- cron, hook, or automation sessions;
- sandboxed sessions;
- subagents or non-primary agent contexts;
- malformed or ambiguous session identifiers;
- absent host session metadata.

Assert the actual `where` object passed to RushDB, not only the final empty result.

## 3. Capture Tests

Given mixed message history containing system, user, assistant, and tool messages:

- capture only the latest completed user/assistant pair;
- ignore partial/failed turns;
- bound and normalize text;
- exclude tool inputs/results and system/developer content;
- avoid writing when no valid pair exists;
- keep deterministic identity stable across retries.

Add explicit fixtures containing fake secrets, command output, and local paths and assert they do not enter automatically captured fields.

## 4. Recall Tests

- Search `EPISODE.summary` and active `MEMORY_FACT.text` separately.
- Apply the complete trusted scope before similarity ranking.
- Exclude the current session when configured.
- Cap query length, result count, formatted context length, and timeout.
- Deduplicate remote and recent-cache results by event/fact ID.
- Return empty context rather than failing the host turn when RushDB errors or times out.
- Mark recalled content as historical, untrusted context rather than instructions.

## 5. Delivery Tests

- Append to the outbox before scheduling remote persistence.
- Retry a transient failure without losing or duplicating the logical event.
- Restart with pending entries and replay them.
- Leave an entry on disk after shutdown timeout.
- Remove an entry only after remote success.
- Bound queue size and worker concurrency.
- Verify malformed outbox entries are isolated without blocking valid entries.

Use temporary profile directories; never test against a user's real host home.

## 6. Lifecycle Tests

| Hook           | Assertion                                                                  |
| -------------- | -------------------------------------------------------------------------- |
| Startup        | Writer starts, pending entries replay, index setup failure degrades safely |
| Pre-inference  | Recall observes timeout and never blocks the host indefinitely             |
| Completed turn | Exactly one bounded episode is enqueued for a successful primary turn      |
| Explicit write | Trusted add/replace produces a fact; unsupported actions do not            |
| Pre-compaction | Last eligible pair is queued without persisting the full transcript        |
| Session switch | Session identity and turn counters change; stale prefetch is cleared       |
| Session end    | Bounded flush runs and session caches are released                         |
| Shutdown       | Worker stops within the budget; pending events remain durable              |

## 7. Compatibility Checks

- Compile or import against the exact pinned runtime dependency.
- Exercise the packaged artifact, not only source-tree imports.
- Verify plugin/provider discovery metadata and entry points.
- Build an installable package and load it in an isolated environment.
- Record the tested runtime version range.
- Re-run lifecycle tests before widening that range.

For an additive provider, verify the host's existing local memory remains enabled. For an exclusive provider, require explicit user selection and document the replacement behavior.
