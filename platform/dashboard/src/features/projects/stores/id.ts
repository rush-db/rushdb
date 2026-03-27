import type { DBRecord } from '@rushdb/javascript-sdk'

import { atom } from 'nanostores'

import type { Project } from '../types'

export const $currentProjectId = atom<Project['id'] | undefined>(undefined)

export const $currentRecordId = atom<DBRecord['__id'] | undefined>(undefined)

export const $sheetRecordId = atom<DBRecord['__id'] | undefined>(undefined)

export type PropertySheetSelection = {
  key: string
  name: string
  type: string
  labels: string[]
  vectorIndexed: boolean
  connectedRecordIds: string[]
}

export const $sheetProperty = atom<PropertySheetSelection | undefined>(undefined)
