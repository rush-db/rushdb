import type { MemoryScope } from './types.js'

export function assertMemoryScope(scope: MemoryScope): void {
  for (const [name, value] of Object.entries({
    agentId: scope.agentId,
    profileId: scope.profileId,
    participantScopeHash: scope.participantScopeHash
  })) {
    if (!value.trim()) {
      throw new Error(`${name} is required for a scoped memory operation`)
    }
  }
}

export function buildScopeWhere(
  scope: MemoryScope,
  options: { excludeSessionId?: string; activeOnly?: boolean } = {}
): Record<string, unknown> {
  assertMemoryScope(scope)

  return {
    agentId: scope.agentId,
    profileId: scope.profileId,
    privacyScope: scope.privacyScope,
    participantScopeHash: scope.participantScopeHash,
    sandboxEligible: scope.sandboxEligible,
    ...(options.excludeSessionId ? { externalSessionId: { $ne: options.excludeSessionId } } : {}),
    ...(options.activeOnly ? { active: true } : {})
  }
}
