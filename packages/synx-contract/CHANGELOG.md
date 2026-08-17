# Changelog

## Unreleased

### Added — v1 contract (private, unpublished)

First cut of the private Cloud destination contract, per the `synx-moat` plan
(Phase 0, Epic 0A). Private package; never published, never imported by the
public SDK/dashboard, never built into the platform image.

- Envelope, acknowledgement, capabilities, connector-descriptor, errors,
  version-negotiation, and canonical hashing modules under `src/`.
- JSON Schemas (draft 2020-12) for the envelope, acknowledgement, capabilities,
  and connector-descriptor payloads; schema set `SCHEMA_HASH_V1` is
  `82d630069b45c974dd9c8986847b8e7c4532db613345b8313c8db15c3400b120`.
- Valid/invalid fixture corpus (29 fixtures) covering envelope batch/operation
  rules, ack status/error rules, capabilities and connector rules, plus
  canonical-JSON and source-identity-hash fixtures.
- Conformance suite (`pnpm test`) validating fixtures with Ajv, checking hash
  stability and fixture↔hash agreement, exercising version negotiation, and
  validating typed values.
- Connector catalog / control plane: `SynxConnectorV1` + fields + capabilities
  descriptors so Cloud surfaces newly deployed connectors in the UI without a
  rushdb redeploy; the catalog is shape-only (never config values/secrets).
  Route: `GET /_internal/synx/connectors` (Synx → Cloud).
- Decisions: open-source self-hosted deployments expose no Synx core API
  (import API / SDKs / Debezium CDC sink only); the Synx connectors, superglue,
  and webhooks are Cloud/enterprise; the contract stays open as the fork-friendly
  vocabulary.
- `pnpm generate:rust` produces the mirrored `synx-contract` Rust crate in
  `rushdb-synx` (generated serde types + `resources/`), run through `rustfmt`;
  `--check` fails when committed output is stale.

Decisions recorded:

- Sequence is strictly increasing per `(binding, stream)`; Core accepts exactly
  the next committed sequence; workers allocate while holding a fenced lease.
- Checkpoints advance only when ack `status == committed` and
  `checkpointAccepted == true`.
- v1 limits: 500 operations / 5 MiB per batch (configurable by measured
  performance).
- Canonical hash contract covers integers, strings, booleans, null, arrays, and
  objects; non-integer floats are excluded (Rust panics rather than diverges).
