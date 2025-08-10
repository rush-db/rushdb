import type { ISO8601 } from '~/types'

export type ProjectLimits = Record<string, number | string>
export type ProjectStats = Record<'records' | 'properties', number>

export type Project = {
  created: ISO8601
  description: string
  id: string
  name: string
  stats?: ProjectStats
  // customDb?: string
  managedDb?: boolean
  limits: string
  validTill?: string
  planId?: string
  productId?: string
  priceId?: string
  isSubscriptionCancelled?: boolean
}

export type WithProjectID = { projectId: Project['id'] }

// TODO: MOVE TO SEARCHPARAMS
export type RecordViewType = 'graph' | 'table' | 'raw-api'

export enum ERecordSheetTabs {
  api = 'API',
  data = 'DATA',
  relations = 'RELATIONS'
}

export type RawApiEntityType = 'records' | 'properties' | 'labels'
