import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'
import { createEpisodeEvent, episodeEventId, stableStringify } from './canonical.js'
import type { AgentMemoryEpisodeInput } from './types.js'

describe('AgentMemoryEvent v1 canonical identity', () => {
  it('is stable across object key ordering', () => {
    expect(stableStringify({ b: 2, a: { d: 4, c: 3 } })).toBe('{"a":{"c":3,"d":4},"b":2}')
  })

  it('matches the language-neutral conformance fixture', () => {
    const fixturePath = fileURLToPath(new URL('../fixtures/conformance.v1.json', import.meta.url))
    const fixture = JSON.parse(readFileSync(fixturePath, 'utf8')) as {
      episodeInput: AgentMemoryEpisodeInput
      expectedEventId: string
    }
    expect(episodeEventId(fixture.episodeInput)).toBe(fixture.expectedEventId)
    expect(createEpisodeEvent(fixture.episodeInput).schemaVersion).toBe(1)
  })
})
