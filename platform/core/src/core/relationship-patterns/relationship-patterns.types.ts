import type { Where } from '@/core/common/types'

export type RelationshipPatternStatus = 'suggested' | 'approved' | 'ignored' | 'error'
export type RelationshipPatternOrigin = 'llm' | 'manual'
export type RelationshipPatternDirection = 'in' | 'out'
export type RelationshipPatternMode = 'join_pattern' | 'retype_existing_relationship'

export type RelationshipPatternEndpoint = {
  label: string
  key?: string
  where?: Where
}

export type RelationshipPatternDto = {
  id: string
  status: RelationshipPatternStatus
  origin: RelationshipPatternOrigin
  source: RelationshipPatternEndpoint
  target: RelationshipPatternEndpoint
  direction: RelationshipPatternDirection
  type: string
  mode: RelationshipPatternMode
  confidence: number
  rationale?: string
  sampleMatchCount?: number
  lastAppliedAt?: string
  lastAnalyzedAt?: string
  lastError?: string
  createdAt: string
  updatedAt: string
}

export type RelationshipPatternCandidate = {
  source: RelationshipPatternEndpoint
  target: RelationshipPatternEndpoint
  direction?: RelationshipPatternDirection
  type?: string
  mode?: RelationshipPatternMode
  confidence?: number
  rationale?: string
  sampleMatchCount?: number
}

export type RelationshipPatternListResponse = {
  patterns: RelationshipPatternDto[]
  relationships: Array<{
    label: string
    relationships: Array<{ label: string; type: string; direction: string }>
  }>
  analysis?: {
    status: string
    requestedAt?: string
    notBefore?: string
    lastRunAt?: string
    lastError?: string
  }
}
