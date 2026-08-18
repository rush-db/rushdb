import { describe, expect, it, vi } from 'vitest'
import { RushDBAgentMemory } from './client.js'
import { createEpisodeEvent } from './canonical.js'

function fakeDB() {
  const upsert = vi.fn(async () => ({ data: {}, exists: true }))
  const vectorSearch = vi.fn(async () => ({ data: [], total: 0 }))
  const find = vi.fn(async () => ({ data: [], total: 0 }))
  return {
    records: { upsert, vectorSearch, find },
    ai: {
      indexes: {
        find: vi.fn(async () => ({ data: [], success: true })),
        create: vi.fn(async () => ({ data: {}, success: true }))
      }
    }
  }
}

const episodeInput = {
  runtime: 'openclaw' as const,
  agentId: 'main',
  profileId: 'default',
  externalSessionId: 'session-1',
  sourceEventId: 'run-1',
  turnIndex: 0,
  userText: 'Use TypeScript',
  assistantText: 'Understood',
  summary: 'The project uses TypeScript',
  conversationKind: 'direct' as const,
  privacyScope: 'private' as const,
  participantScopeHash: 'scope-a',
  sandboxEligible: false,
  visibility: 'participant' as const,
  trustClass: 'mixed' as const,
  originClass: 'conversation',
  observedAt: '2026-08-18T00:00:00Z',
  provenance: 'openclaw:agent_end'
}

describe('RushDBAgentMemory', () => {
  it('persists deterministic events with mergeBy idempotency', async () => {
    const db = fakeDB()
    const memory = new RushDBAgentMemory({ db: db as never })
    const event = createEpisodeEvent(episodeInput)
    await memory.persistEpisode(event)
    expect(db.records.upsert).toHaveBeenCalledWith({
      label: 'EPISODE',
      data: event,
      options: { mergeBy: ['eventId'], mergeStrategy: 'append' }
    })
  })

  it('puts every scope field into each semantic query', async () => {
    const db = fakeDB()
    const memory = new RushDBAgentMemory({ db: db as never })
    await memory.recall({
      query: 'language choice',
      agentId: 'main',
      profileId: 'default',
      privacyScope: 'private',
      participantScopeHash: 'scope-a',
      sandboxEligible: false,
      excludeSessionId: 'current'
    })
    expect(db.records.vectorSearch).toHaveBeenNthCalledWith(
      1,
      expect.objectContaining({
        labels: ['EPISODE'],
        where: {
          agentId: 'main',
          profileId: 'default',
          privacyScope: 'private',
          participantScopeHash: 'scope-a',
          sandboxEligible: false,
          externalSessionId: { $ne: 'current' }
        }
      })
    )
    expect(db.records.vectorSearch).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({
        labels: ['MEMORY_FACT'],
        where: expect.objectContaining({ active: true, participantScopeHash: 'scope-a' })
      })
    )
  })

  it('recalls a recent write before vector indexing catches up', async () => {
    const db = fakeDB()
    const memory = new RushDBAgentMemory({ db: db as never })
    memory.rememberRecent(episodeInput)
    const recalled = await memory.recall({
      query: 'TypeScript project',
      agentId: 'main',
      profileId: 'default',
      privacyScope: 'private',
      participantScopeHash: 'scope-a',
      sandboxEligible: false,
      includeFacts: false
    })
    expect(recalled).toHaveLength(1)
    expect(recalled[0]?.text).toContain('TypeScript')
  })
})
