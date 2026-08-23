import {
  buildIdempotencyKey,
  measureRelationships,
  measureWrite,
  scalarValueBytes
} from '@/core/usage-events/write-measurements'

describe('write-measurements', () => {
  describe('scalarValueBytes', () => {
    it('measures utf8 byte length of strings', () => {
      expect(scalarValueBytes('abc')).toBe(3)
      expect(scalarValueBytes('é')).toBe(2) // 2-byte utf8 code unit
    })

    it('measures numbers and booleans as small scalars', () => {
      expect(scalarValueBytes(42)).toBe(8)
      expect(scalarValueBytes(true)).toBe(1)
    })

    it('gives zero weight to null, undefined and non-scalars', () => {
      expect(scalarValueBytes(null)).toBe(0)
      expect(scalarValueBytes(undefined)).toBe(0)
      expect(scalarValueBytes({ nested: true })).toBe(0)
      expect(scalarValueBytes([1, 2])).toBe(0)
    })
  })

  describe('measureWrite', () => {
    it('counts scalar values and their serialized bytes', () => {
      const result = measureWrite({
        name: 'Acme',
        plan: 'pro',
        active: true,
        description: 'a'.repeat(10_000),
        rating: 4.5
      })

      expect(result.scalarValuesWritten).toBe(5)
      expect(result.valueBytesWritten).toBe(4 + 3 + 1 + 10_000 + 8)
    })

    it('ignores nested objects (decomposed into separate records by RushDB)', () => {
      const result = measureWrite({ name: 'Acme', metadata: { deep: 'value' } })

      expect(result.scalarValuesWritten).toBe(1)
      expect(result.valueBytesWritten).toBe(4)
    })

    it('skips null and undefined values entirely', () => {
      expect(measureWrite({ a: null, b: undefined, c: 1 })).toEqual({
        scalarValuesWritten: 1,
        valueBytesWritten: 8
      })
    })

    it('reports exact byte counts', () => {
      expect(measureWrite({ v: 'x' }).valueBytesWritten).toBe(1)
      expect(measureWrite({ v: 'x'.repeat(1_000_000) }).valueBytesWritten).toBe(1_000_000)
    })
  })

  describe('measureRelationships', () => {
    it('passes through non-negative counts and clamps negatives to zero', () => {
      expect(measureRelationships(7)).toBe(7)
      expect(measureRelationships(0)).toBe(0)
      expect(measureRelationships(-3)).toBe(0)
      expect(measureRelationships(2.9)).toBe(2)
    })
  })

  describe('buildIdempotencyKey', () => {
    it('is stable for identical inputs', () => {
      const a = buildIdempotencyKey('ws1', 'p1', 'req1', '/records/search')
      const b = buildIdempotencyKey('ws1', 'p1', 'req1', '/records/search')
      expect(a).toBe(b)
      expect(a).toMatch(/^[a-f0-9]{64}$/)
    })

    it('differs across workspaces, requests and operation keys', () => {
      const base = buildIdempotencyKey('ws1', 'p1', 'req1', 'write')
      expect(buildIdempotencyKey('ws2', 'p1', 'req1', 'write')).not.toBe(base)
      expect(buildIdempotencyKey('ws1', 'p1', 'req2', 'write')).not.toBe(base)
      expect(buildIdempotencyKey('ws1', 'p1', 'req1', 'ingestion')).not.toBe(base)
    })
  })
})
