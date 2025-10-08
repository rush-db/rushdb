import type { ISO8601 } from '~/types'

export type ProjectStats = Record<'records' | 'properties' | 'avgProperties', number>

type ProjectStatus = 'pending' | 'active' | 'provisioning' | 'suspended'

export type Project = {
  created: ISO8601
  description: string
  id: string
  name: string
  stats?: ProjectStats
  isSubscriptionCancelled?: boolean
  customDb?: string
  managedDbRegion?: string
  managedDbTier?: string
  status?: ProjectStatus
  validTill?: string
  planId?: string
  productId?: string
  priceId?: string
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
