import type { SearchQuery } from '@rushdb/javascript-sdk'

export type RelationshipPatternStatus = 'suggested' | 'approved' | 'ignored' | 'error'
export type RelationshipPatternDirection = 'in' | 'out'
export type RelationshipPatternMode = 'join_pattern' | 'retype_existing_relationship'

export type RelationshipPatternEndpoint = {
  label: string
  key?: string
  where?: SearchQuery['where']
}

export type RelationshipPattern = {
  id: string
  status: RelationshipPatternStatus
  origin: 'llm' | 'manual'
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

export type ExistingRelationshipSummary = {
  label: string
  relationships: Array<{ label: string; type: string; direction: string }>
}

export type RelationshipPatternsResponse = {
  patterns: RelationshipPattern[]
  relationships: ExistingRelationshipSummary[]
  analysis?: {
    status: string
    requestedAt?: string
    notBefore?: string
    lastRunAt?: string
    lastError?: string
  }
}
