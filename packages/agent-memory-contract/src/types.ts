export const AGENT_MEMORY_SCHEMA_VERSION = 1 as const

export type AgentMemoryRuntime = 'openclaw' | 'hermes' | 'custom'
export type MemoryPrivacyScope = 'private' | 'shared'
export type MemoryConversationKind = 'direct' | 'local' | 'shared'
export type MemoryVisibility = 'participant' | 'profile' | 'shared'
export type MemoryTrustClass = 'trusted' | 'mixed' | 'untrusted'

export interface MemoryScope {
  agentId: string
  profileId: string
  privacyScope: MemoryPrivacyScope
  participantScopeHash: string
  sandboxEligible: boolean
}

export interface AgentMemoryEpisodeInput extends MemoryScope {
  runtime: AgentMemoryRuntime
  externalSessionId: string
  sourceEventId?: string
  turnIndex: number
  userText: string
  assistantText: string
  summary: string
  conversationKind: MemoryConversationKind
  channelIdHash?: string
  visibility: MemoryVisibility
  trustClass: MemoryTrustClass
  originClass: string
  observedAt: string
  provenance: string
}

export interface AgentMemoryEpisodeV1 extends AgentMemoryEpisodeInput {
  schemaVersion: typeof AGENT_MEMORY_SCHEMA_VERSION
  eventType: 'episode'
  eventId: string
}

export interface AgentMemoryFactInput extends MemoryScope {
  runtime: AgentMemoryRuntime
  text: string
  kind: string
  subjectKey: string
  confidence: number
  active: boolean
  sourceEventId: string
  supersedesFactId?: string
  validFrom: string
  validUntil?: string
  visibility: MemoryVisibility
  trustClass: MemoryTrustClass
  provenance: string
}

export interface AgentMemoryFactV1 extends AgentMemoryFactInput {
  schemaVersion: typeof AGENT_MEMORY_SCHEMA_VERSION
  eventType: 'fact'
  factId: string
}

export type AgentMemoryEventV1 = AgentMemoryEpisodeV1 | AgentMemoryFactV1

export interface RecalledMemory {
  id: string
  label: 'EPISODE' | 'MEMORY_FACT'
  text: string
  score: number
  observedAt?: string
  provenance?: string
  trustClass?: MemoryTrustClass
}

export interface RecallOptions extends MemoryScope {
  query: string
  limit?: number
  excludeSessionId?: string
  includeFacts?: boolean
}
