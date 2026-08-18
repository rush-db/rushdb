# AgentMemoryEvent v1

Use the published JSON Schema from `@rushdb/agent-memory-contract/schema`. Do not create a similar host-specific event format.

## Canonical Records

### Episode

An episode contains a bounded completed user/assistant pair, a summary, runtime/session identity, turn index, timestamp, provenance, trust metadata, and the required scope fields. Compute `eventId` from canonical JSON using the contract package.

Persist with:

```ts
await db.records.upsert({
  label: 'EPISODE',
  data: event,
  options: { mergeBy: ['eventId'], mergeStrategy: 'append' }
})
```

### Fact

A fact contains `text`, `kind`, `subjectKey`, confidence, validity, provenance, trust metadata, source event identity, and the required scope fields. Compute `factId` through the contract package.

Normal recall must include `active: true`. Replacing a fact creates a new active fact, deactivates the prior fact, and sets `supersedesFactId` on the new version.

## Required Scope

Every episode and fact carries:

```text
agentId
profileId
privacyScope
participantScopeHash
sandboxEligible
```

Derive scope from trusted host metadata. Hash participant identifiers with a deployment-specific salt. Treat scope as an authorization prefilter, never a ranking hint.

## Trust Boundary

Record `provenance`, `trustClass`, `originClass` where applicable, and `visibility`. Format recall as quoted contextual data with an explicit statement that it is not instructions or policy.

## Capture Boundary

Capture only bounded user and assistant text required for the episode. Exclude system prompts, complete message arrays, tool calls/results, secrets, command output, and local paths.

## Delivery Semantics

- Use deterministic IDs and idempotent upsert for at-least-once delivery.
- Do not keep a database transaction open across inference.
- Use a durable local outbox for background persistence.
- Keep a bounded recent-write cache or exact lookup while embeddings become visible.
