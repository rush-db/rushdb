# `@rushdb/synx-contract`

**Private.** The Cloud destination contract between RushDB Synx workers and the
RushDB Cloud Core destination. This package is never published, never imported
by the public SDK or the dashboard, and never built into the platform image. It
exists as the single source of truth for the wire contract.

One envelope targets exactly one Cloud source binding and one stream. It is the
single batch unit exchanged via `POST /api/v1/_internal/synx/batches`; Core
responds with an acknowledgement, and Synx advances its source checkpoint only
when the acknowledgement status is `committed` and `checkpointAccepted` is
`true`.

## Cross-repo mirror

The same contract is enforced on the Synx side (Rust) and the Core side
(TypeScript). This package holds the TypeScript definition, the JSON Schemas,
the fixture corpus, and the generator that produces the mirrored Rust crate:

```
rushdb/packages/synx-contract/  →  rushdb-synx/crates/synx-contract/
  schema/*.json                   →  resources/schema/*.json
  fixtures/**                     →  resources/fixtures/**
  src/generated.rs (generated)    ←  scripts/generate-rust.ts
```

Regenerate the mirror after any schema or fixture change:

```bash
pnpm generate:rust       # write into ../../rushdb-synx/crates/synx-contract
pnpm generate:rust --check
```

`--check` fails unless the committed Rust output is byte-identical to a fresh
generation. Generated Rust is run through `rustfmt` so the Synx CI
`cargo fmt --check` gate stays green.

## Layout

| Path                       | Contents                                                                                            |
| -------------------------- | --------------------------------------------------------------------------------------------------- |
| `src/envelope.ts`          | `SynxEnvelopeV1`, `SynxSourceV1`, `SynxStreamV1`, `SynxBatchV1`, `SynxMappingV1`, `SynxOperationV1` |
| `src/acknowledgement.ts`   | `SynxAcknowledgementV1`, `SynxAckStatusV1`, `SynxAckErrorV1`                                        |
| `src/capabilities.ts`      | `SynxCapabilitiesV1`, `SynxContractLimitsV1`                                                        |
| `src/connectors.ts`        | `SynxConnectorV1`, `SynxConnectorCapabilitiesV1`, `SynxConnectorFieldV1`, `SynxConnectorCatalogV1`  |
| `src/errors.ts`            | `SYNX_ERROR_CODES`, `isRetryableCode`, `SynxErrorResponseV1`                                        |
| `src/version.ts`           | `CONTRACT_VERSION`, `negotiateContract`, `compareVersion`                                           |
| `src/hash.ts`              | canonical JSON + source-identity + schema-set hashing                                               |
| `src/schemas.ts`           | `SCHEMAS_V1` (schema set keyed by canonical filename)                                               |
| `schema/*.json`            | JSON Schemas (draft 2020-12)                                                                        |
| `fixtures/`                | valid / invalid fixtures per payload + hash fixtures                                                |
| `scripts/conformance.ts`   | `pnpm test` — fixture + hash + negotiation + typed checks                                           |
| `scripts/generate-rust.ts` | `pnpm generate:rust` / `--check`                                                                    |

## Wire format

`Content-Type: application/vnd.rushdb.synx+json;version=1`

| Payload         | Route                                     | Direction                           |
| --------------- | ----------------------------------------- | ----------------------------------- |
| envelope        | `POST /api/v1/_internal/synx/batches`     | Synx → Core                         |
| acknowledgement | response body of the above                | Core → Synx                         |
| capabilities    | `GET /api/v1/_internal/synx/capabilities` | Core → Synx (service identity only) |
| connector       | `GET /api/v1/_internal/synx/connectors`   | Synx → Cloud (control plane)        |

Core routes are service-authenticated (mTLS / control-plane credential), never
user-token. The source binding and batch identity come from the caller's
credentials and the request path — never from the body.

## Two planes

- **Data plane** (Synx → Core): the envelope / acknowledgement / checkpoint
  flow above. This surface lives in the Cloud Core destination and is open for
  forks to rely on with their own Synx implementations.
- **Connector catalog / control plane** (Synx → Cloud): `SynxConnectorV1`
  descriptors let Cloud discover available source connectors at runtime — schema
  and config fields — so adding a new connector to a deployed Synx instance
  surfaces it in the UI without a rushdb redeploy. The catalog exposes _shape_
  only (labels, field schemas, capabilities), never stored config values or
  secrets.

Open-source self-hosted deployments expose no Synx-specific API: integration is
via the import API / SDK and the published Debezium CDC sink. The Synx
connectors, superglue, and webhooks are Cloud/enterprise, backed by this
contract.

## Semantics

- **Sequence** — strictly increasing per `(binding, stream)`. Core accepts
  exactly the next committed sequence; a gap or stale sequence is rejected with
  a stable `409 SEQUENCE_GAP` / `SEQUENCE_STALE`. A worker allocates a sequence
  only while holding a current fenced lease; a replay after a lost ack must not
  skip a sequence.
- **Batch modes** — `snapshot` (initial load), `incremental` (steady state),
  `replay` (recovery/resync). `deletionMode` (`ignore` / `soft_delete` /
  `hard_delete`) and `mappingVersion` come from the approved mapping.
- **Acknowledgements** — `committed` means the batch is durable and the source
  offset may advance; `rejected` returns per-operation errors (`operationIndex`,
  `sourceIdHash`, code, `retryable`); `duplicate` means the batch id was already
  applied (safe to treat as committed). Checkpoints advance only when
  `status == committed && checkpointAccepted == true`.

## Version policy

- Additive optional fields stay compatible within v1.
- Enum expansion is breaking unless both services negotiate the new version
  first.
- Services negotiate supported versions during health/claim and never silently
  downgrade. `negotiateContract` picks the highest mutual version and returns
  `null` when none exists (a hard deployment error). Core deploys support before
  Synx emits a new version.

## Limits (v1)

Hard, configurable by measured Cloud performance: `maxOperationsPerBatch` 500,
`maxBytesPerBatch` 5 MiB.

## Error codes

`INVALID_CONTRACT_VERSION`, `INVALID_BINDING`, `BINDING_INACTIVE`,
`ENTITLEMENT_REJECTED`, `SEQUENCE_GAP`, `SEQUENCE_STALE`,
`MALFORMED_OPERATION`, `MAPPING_VERSION_STALE`, `SCHEMA_VERSION_STALE`,
`LIMIT_EXCEEDED`, `RATE_LIMITED`, `UPSTREAM_UNAVAILABLE`, `INTERNAL`.

Codes are additive; unknown codes must be treated as non-retryable.
`RATE_LIMITED`, `UPSTREAM_UNAVAILABLE`, and `INTERNAL` are retryable by default;
`retryable` on an error detail overrides the code-level default.

## Canonical hashing

`hash.ts` / the Rust `hash` module must stay byte-identical:

- Canonical JSON: object keys sorted recursively, no insignificant whitespace,
  JS-style string escaping.
- `hashSourceIdentity(parts)` — SHA-256 hex of the canonical parts array
  (e.g. `[streamName, ...pkValues]`). Core stores this as the internal record
  identity; Synx uses it to correlate.
- `computeSchemaHash(schemas)` — SHA-256 of `{ [filename]: schema }` with
  filename keys sorted, so declaration order never matters.

The canonical contract covers integer numbers, strings, booleans, null, arrays,
and objects. Non-integer floats are not part of the contract; the Rust side
panics on them rather than silently diverging.

## Development

```bash
pnpm types:check                 # tsc --noEmit
pnpm test                        # conformance: 22 fixtures + hashes + negotiation
pnpm generate:rust               # regenerate the rushdb-synx mirror
pnpm generate:rust --check       # fail unless committed output is up to date
```

Formatting follows the repo root prettier config (via lint-staged on
`packages/**/*.{ts,tsx}` and `**/*.json`).
