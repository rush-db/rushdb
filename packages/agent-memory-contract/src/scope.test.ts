import { describe, expect, it } from 'vitest'
import { buildScopeWhere } from './scope.js'

describe('scope-safe query construction', () => {
  const scope = {
    agentId: 'research',
    profileId: 'default',
    privacyScope: 'private' as const,
    participantScopeHash: 'abc',
    sandboxEligible: false
  }

  it('always includes every security scope field', () => {
    expect(buildScopeWhere(scope)).toEqual(scope)
  })

  it('adds lifecycle filters without allowing scope override', () => {
    expect(buildScopeWhere(scope, { excludeSessionId: 'current', activeOnly: true })).toEqual({
      ...scope,
      externalSessionId: { $ne: 'current' },
      active: true
    })
  })
})
