import { decideSequence, parseOffsetPosition, stringifyOffsetCheckpoint, synxPartition } from './synx.offsets'

describe('SynxOffsetCheckpoint helpers', () => {
  it('scopes partitions by binding and stream', () => {
    expect(synxPartition('bnd-1', 'stream-1')).toBe('synx:bnd-1:stream-1')
  })

  it('round-trips a checkpoint through the offset position column', () => {
    const checkpoint = { sequence: 7, updatedAt: '2026-01-01T00:00:00.000Z' }
    const decoded = parseOffsetPosition(stringifyOffsetCheckpoint(checkpoint))
    expect(decoded).toEqual(checkpoint)
  })

  it('returns null for missing or malformed positions', () => {
    expect(parseOffsetPosition(null)).toBeNull()
    expect(parseOffsetPosition('')).toBeNull()
    expect(parseOffsetPosition('not json')).toBeNull()
  })

  it('accepts exactly the next committed sequence', () => {
    expect(decideSequence(null, 0)).toBe('accept')
    expect(decideSequence({ sequence: 3, updatedAt: '' }, 4)).toBe('accept')
  })

  it('classifies stale sequences as duplicate', () => {
    expect(decideSequence({ sequence: 3, updatedAt: '' }, 2)).toBe('duplicate')
    expect(decideSequence({ sequence: 3, updatedAt: '' }, 3)).toBe('duplicate')
  })

  it('classifies skipped sequences as a gap', () => {
    expect(decideSequence(null, 5)).toBe('gap')
    expect(decideSequence({ sequence: 3, updatedAt: '' }, 5)).toBe('gap')
  })
})
