/**
 * Conformance suite for the private Cloud destination contract (v1).
 *
 * Verifies:
 *  1. every valid fixture validates against its JSON Schema;
 *  2. every invalid fixture fails its JSON Schema;
 *  3. the schema set hash is stable and well-formed;
 *  4. canonical JSON / source-identity hash fixtures agree with `hash.ts`;
 *  5. version negotiation behaves per the contract version policy;
 *  6. values constructed from the TypeScript types validate against the schemas.
 *
 * Run: `pnpm test` (from packages/synx-contract).
 */

import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, dirname, relative } from 'node:path'
import { fileURLToPath } from 'node:url'
import Ajv2020 from 'ajv/dist/2020'
import { type AnySchema, type ValidateFunction } from 'ajv'

import {
  CONTRACT_VERSION,
  computeSchemaHash,
  catalogForPlan,
  canonicalJsonHash,
  canonicalJsonString,
  hashSourceIdentity,
  negotiateContract,
  SCHEMAS_V1,
  type SynxEnvelopeV1,
  type SynxConnectorV1
} from '../src/index'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')

function fail(message: string): never {
  throw new Error(`CONFORMANCE FAILURE: ${message}`)
}

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) {
    fail(message)
  }
}

function loadJson(relativePath: string): unknown {
  return JSON.parse(readFileSync(join(root, relativePath), 'utf8'))
}

function walk(dir: string): string[] {
  const out: string[] = []
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry)
    if (statSync(full).isDirectory()) {
      out.push(...walk(full))
    } else if (full.endsWith('.json')) {
      out.push(full)
    }
  }
  return out
}

const SCHEMAS: Record<string, AnySchema> = SCHEMAS_V1 as Record<string, AnySchema>

function validatorsFor(compiler: Ajv2020): Record<string, ValidateFunction> {
  return {
    envelope: compiler.compile(SCHEMAS['synx-envelope-v1.schema.json']),
    acknowledgement: compiler.compile(SCHEMAS['synx-ack-v1.schema.json']),
    capabilities: compiler.compile(SCHEMAS['synx-capabilities-v1.schema.json']),
    connector: compiler.compile(SCHEMAS['synx-connector-v1.schema.json'])
  }
}

const ajv = new Ajv2020({ strict: false, validateFormats: false })
const validators = validatorsFor(ajv)

let checked = 0

function checkFixture(relPath: string, expectValid: boolean): void {
  const value = loadJson(relPath)
  const segment = relPath.split('/')[2]
  const key =
    segment === 'envelope' ? 'envelope'
    : segment === 'acknowledgement' ? 'acknowledgement'
    : segment === 'capabilities' ? 'capabilities'
    : 'connector'
  const validate = validators[key]
  if (!validate) {
    fail(`unknown fixture category in ${relPath}`)
  }
  const ok = validate(value) === true
  checked += 1
  if (expectValid && !ok) {
    fail(`expected VALID but failed: ${relPath}\n  ${JSON.stringify(validate.errors)}`)
  }
  if (!expectValid && ok) {
    fail(`expected INVALID but passed: ${relPath}`)
  }
}

// 1 & 2. fixture validity
for (const file of walk(join(root, 'fixtures', 'valid'))) {
  checkFixture(relative(root, file), true)
}
for (const file of walk(join(root, 'fixtures', 'invalid'))) {
  checkFixture(relative(root, file), false)
}

// 3. schema set hash
const schemaHash = computeSchemaHash(SCHEMAS_V1)
assert(
  typeof schemaHash === 'string' && /^[0-9a-f]{64}$/.test(schemaHash),
  `unexpected schema hash shape: ${schemaHash}`
)
// hash must be stable (recompute == cached if we ever cache it) and independent of declaration order
const reordered: Record<string, unknown> = {}
for (const key of Object.keys(SCHEMAS_V1).reverse()) {
  reordered[key] = SCHEMAS_V1[key]
}
assert(schemaHash === computeSchemaHash(reordered), 'schema hash must be independent of key order')

// the valid capabilities fixture must carry the real schema-set hash Core would emit
const capabilitiesFixture = loadJson('fixtures/valid/capabilities/defaults.json') as { schemaHash?: string }
assert(
  capabilitiesFixture.schemaHash === schemaHash,
  `capabilities fixture schemaHash must equal the computed schema set hash: got ${capabilitiesFixture.schemaHash}, want ${schemaHash}`
)

// 4. canonical JSON + identity hash fixtures
const hashFixtures = loadJson('fixtures/hash/identity-hash.fixtures.json') as {
  canonicalJson: Array<{ input: unknown; expected: string }>
  sourceIdentityHash: Array<{ parts: unknown[]; expected: string }>
}
for (const fixture of hashFixtures.canonicalJson) {
  assert(
    canonicalJsonString(fixture.input) === fixture.expected,
    `canonicalJsonString mismatch for ${JSON.stringify(fixture.input)}: got ${canonicalJsonString(fixture.input)}, want ${fixture.expected}`
  )
}
for (const fixture of hashFixtures.sourceIdentityHash) {
  const actual = hashSourceIdentity(fixture.parts)
  assert(
    actual === fixture.expected,
    `hashSourceIdentity mismatch for ${JSON.stringify(fixture.parts)}: got ${actual}, want ${fixture.expected}`
  )
}
assert(canonicalJsonHash({ a: 1 }) === canonicalJsonHash({ a: 1 }), 'canonicalJsonHash must be deterministic')

// 5. version negotiation
assert(CONTRACT_VERSION === '1', 'CONTRACT_VERSION must be "1"')
assert(negotiateContract(['1'], ['1']) === '1', 'mutual v1 negotiates to v1')
assert(negotiateContract(['2', '1'], ['1']) === '1', 'highest mutual version is chosen')
assert(negotiateContract(['2'], ['1']) === null, 'no shared version returns null')
assert(negotiateContract([], ['1']) === null, 'empty client versions returns null')

// 6. typed values validate against the schemas
const typedEnvelope: SynxEnvelopeV1 = {
  version: '1',
  source: { bindingId: 'bind_01HZ2X', connectionId: 'conn_01HZ2W', connectorType: 'mongodb' },
  stream: { id: 'stm_01HZ2Y', name: 'inventory.items' },
  batch: { id: 'b_typed', sequence: 1, emittedAt: '2026-08-02T09:15:00.000Z', mode: 'snapshot' },
  mapping: { targetLabel: 'ORDER', identityFields: ['id'], deletionMode: 'soft_delete', mappingVersion: 1 },
  operations: [
    {
      type: 'upsert',
      sourceId: '42',
      sourceUpdatedAt: '2026-08-02T09:14:59.000Z',
      data: { id: 42 },
      relations: [{ type: 'PLACED_BY', to: { streamId: 'stm_01HZ2A', sourceId: '9' } }]
    },
    { type: 'delete', sourceId: '7' }
  ]
}
assert(
  validators.envelope(JSON.parse(JSON.stringify(typedEnvelope))) === true,
  'typed SynxEnvelopeV1 must validate against the envelope schema'
)

const typedConnector: SynxConnectorV1 = {
  id: 'hubspot',
  type: 'crm',
  name: 'HubSpot',
  version: '1.0.0',
  schemaVersion: '1',
  capabilities: {
    batchModes: ['snapshot', 'incremental'],
    deletionModes: ['soft_delete'],
    relationEvidence: true
  },
  fields: [
    { key: 'apiKey', label: 'API key', type: 'string', required: true, secret: true },
    {
      key: 'ownerType',
      label: 'Owner',
      type: 'select',
      required: false,
      options: [{ value: 'a', label: 'A' }]
    }
  ]
}
assert(
  validators.connector(JSON.parse(JSON.stringify(typedConnector))) === true,
  'typed SynxConnectorV1 must validate against the connector schema'
)

// Entitlement: the provider union + minimum tiers live on the synx side; the
// catalog partitions by the requesting workspace's plan.
const entitledConnector: SynxConnectorV1 = {
  ...typedConnector,
  entitlement: 'paid'
}
assert(
  validators.connector(JSON.parse(JSON.stringify(entitledConnector))) === true,
  'typed SynxConnectorV1 with entitlement must validate against the connector schema'
)

const catalogInput: SynxConnectorV1[] = [
  { ...typedConnector, id: 'free-conn', entitlement: 'free' },
  { ...typedConnector, id: 'paid-conn', entitlement: 'paid' },
  { ...typedConnector, id: 'top-conn', entitlement: 'top_tier' }
]
const freeCatalog = catalogForPlan(catalogInput, 'free')
assert(
  freeCatalog.connectors.map((c) => c.id).join(',') === 'free-conn',
  'free plan sees only free-tier connectors'
)
assert(
  freeCatalog.unavailable
    .map((u) => u.id)
    .sort()
    .join(',') === 'paid-conn,top-conn',
  'free plan sees paid/top-tier connectors as unavailable with reasons'
)
const proCatalog = catalogForPlan(catalogInput, 'pro')
assert(
  proCatalog.connectors.map((c) => c.id).join(',') === 'free-conn,paid-conn',
  'paid plan sees free + paid connectors'
)
assert(
  proCatalog.unavailable.map((u) => u.id).join(',') === 'top-conn',
  'paid plan sees only top-tier as unavailable'
)
const topCatalog = catalogForPlan(catalogInput, 'enterprise')
assert(topCatalog.connectors.length === 3, 'top-tier plan sees every connector')
assert(topCatalog.unavailable.length === 0, 'top-tier plan has no unavailable connectors')

console.log(`synx-contract conformance OK: ${checked} fixtures checked, schemaHash=${schemaHash}`)
