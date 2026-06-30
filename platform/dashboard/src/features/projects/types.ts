import type { ISO8601 } from '~/types'

export type ProjectStats = Record<'records' | 'properties', number>

type ProjectStatus = 'pending' | 'active' | 'provisioning' | 'suspended'

export type Project = {
  created: ISO8601
  description: string
  id: string
  name: string
  stats?: ProjectStats
  customDb?: string
  status?: ProjectStatus
}

export type WithProjectID = { projectId: Project['id'] }

// TODO: MOVE TO SEARCHPARAMS
export type RecordViewType = 'graph' | 'table' | 'json'

export enum ERecordSheetTabs {
  api = 'API',
  data = 'DATA',
  relations = 'RELATIONS'
}

export type RawApiEntityType = 'records' | 'properties' | 'labels'
