import { deriveRunStatus, canTransitionFile } from './domain/import-state-machine'
import {
  suggestLabelFromFileName,
  detectFormatFromFileName,
  looksLikeReferenceColumn,
  suggestRole
} from './domain/label-suggestion'
import { validateLinkColumns, validateLinkSpec, ImportValidationError } from './domain/link-spec.validation'
import { deriveDeterministicRecordId, hashIdempotencyKey } from './domain/replay-identity'

import type { ImportLinkSpec } from './domain/import-run.types'

const baseSpec: ImportLinkSpec = {
  version: 1,
  role: 'links',
  endpoints: [
    { column: 'character_id', label: 'CHARACTER', keyProperty: 'id', direction: 'source' },
    { column: 'episode_id', label: 'EPISODE', keyProperty: 'id', direction: 'target' }
  ],
  relationshipType: 'APPEARED_IN'
}

describe('ImportRuns state machine', () => {
  it('derives completed only when every file completed', () => {
    expect(
      deriveRunStatus([
        { status: 'completed', role: 'records' },
        { status: 'completed', role: 'links' }
      ])
    ).toBe('completed')
  })

  it('derives completed_with_errors on mixed terminal outcomes', () => {
    expect(
      deriveRunStatus([
        { status: 'completed', role: 'records' },
        { status: 'failed', role: 'links' }
      ])
    ).toBe('completed_with_errors')
  })

  it('derives canceled when all files canceled', () => {
    expect(deriveRunStatus([{ status: 'canceled', role: 'records' }])).toBe('canceled')
  })

  it('derives failed when no file completed', () => {
    expect(deriveRunStatus([{ status: 'failed', role: 'records' }])).toBe('failed')
  })

  it('stays running while any file is nonterminal', () => {
    expect(
      deriveRunStatus([
        { status: 'completed', role: 'records' },
        { status: 'running', role: 'links' }
      ])
    ).toBe('running')
  })

  it('forbids transitions into completed from non-terminal states', () => {
    expect(canTransitionFile('queued', 'completed')).toBe(false)
    expect(canTransitionFile('running', 'finalizing')).toBe(true)
    expect(canTransitionFile('completed', 'running')).toBe(false)
  })
})

describe('ImportRuns label suggestion', () => {
  it('suggests labels deterministically from filenames', () => {
    expect(suggestLabelFromFileName('users.csv')).toBe('USER')
    expect(suggestLabelFromFileName('people.ndjson')).toBe('PEOPLE')
    expect(suggestLabelFromFileName('order-items.jsonl')).toBe('ORDER_ITEM')
    expect(suggestLabelFromFileName('char_ep.csv')).toBe('CHAR_EP')
    expect(suggestLabelFromFileName('crm_accounts_2026.csv')).toBe('CRM_ACCOUNTS_2026')
  })

  it('detects supported formats', () => {
    expect(detectFormatFromFileName('a.csv')).toBe('csv')
    expect(detectFormatFromFileName('a.ndjson')).toBe('ndjson')
    expect(detectFormatFromFileName('a.json')).toBe('json')
    expect(detectFormatFromFileName('a.parquet')).toBe('parquet')
    expect(detectFormatFromFileName('a.txt')).toBeNull()
  })

  it('flags reference-like columns and suggests links only for pure join shapes', () => {
    expect(looksLikeReferenceColumn('character_id')).toBe(true)
    expect(looksLikeReferenceColumn('id')).toBe(true)
    expect(looksLikeReferenceColumn('name')).toBe(false)

    const suggestion = suggestRole({ columns: ['character_id', 'episode_id'] })
    expect(suggestion.role).toBe('links')

    const valueRich = suggestRole({ columns: ['character_id', 'name'] })
    expect(valueRich.role).toBe('records')

    const tooMany = suggestRole({ columns: ['a_id', 'b_id', 'c_id'] })
    expect(tooMany.role).toBe('records')
  })
})

describe('ImportRuns replay identity', () => {
  it('produces stable ids across invocations', () => {
    const input = {
      projectId: 'p1',
      fileId: 'f1',
      sourceGeneration: 1,
      unitOrdinal: 42,
      path: '0',
      siblingOccurrence: 0
    }
    expect(deriveDeterministicRecordId(input)).toBe(deriveDeterministicRecordId(input))
  })

  it('differs by project, file, generation, ordinal and path', () => {
    const base = {
      projectId: 'p1',
      fileId: 'f1',
      sourceGeneration: 1,
      unitOrdinal: 1,
      path: '0',
      siblingOccurrence: 0
    }
    const variants = [
      { ...base, projectId: 'p2' },
      { ...base, fileId: 'f2' },
      { ...base, unitOrdinal: 2 },
      { ...base, path: '1' }
    ].map((v) => deriveDeterministicRecordId(v))

    expect(new Set(variants).size).toBe(variants.length)
  })

  it('hashes idempotency keys stably', () => {
    expect(hashIdempotencyKey('abc')).toBe(hashIdempotencyKey('abc'))
    expect(hashIdempotencyKey('abc')).not.toBe(hashIdempotencyKey('abd'))
  })
})

describe('ImportRuns link spec validation', () => {
  it('accepts a valid spec', () => {
    expect(() => validateLinkSpec(baseSpec)).not.toThrow()
  })

  it('rejects missing or malformed specs', () => {
    expect(() => validateLinkSpec(null)).toThrow(ImportValidationError)
    expect(() =>
      validateLinkSpec({
        ...baseSpec,
        endpoints: [baseSpec.endpoints[0], { ...baseSpec.endpoints[1], direction: 'source' }]
      })
    ).toThrow(/source/)
    expect(() => validateLinkSpec({ ...baseSpec, relationshipType: '' })).toThrow(ImportValidationError)
  })

  it('rejects unmapped surplus columns silently being dropped', () => {
    expect(() => validateLinkColumns(baseSpec, ['character_id', 'episode_id'])).not.toThrow()
    expect(() => validateLinkColumns(baseSpec, ['character_id', 'episode_id', 'credit'])).toThrow(/credit/)
    expect(() =>
      validateLinkColumns({ ...baseSpec, propertyColumns: { credit: 'credited' } }, [
        'character_id',
        'episode_id',
        'credit'
      ])
    ).not.toThrow()
  })
})
