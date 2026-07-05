import type { DBRecord, Property } from '@rushdb/javascript-sdk'

import { atom } from 'nanostores'

import type { Project } from '../types'

export const $currentProjectId = atom<Project['id'] | undefined>(undefined)

export const $currentRecordId = atom<DBRecord['__id'] | undefined>(undefined)

export const $sheetRecordId = atom<DBRecord['__id'] | undefined>(undefined)

export type PropertySheetData = Property & {
  vectorIndexed: boolean
  connectedRecordIds: string[]
}

export const $sheetProperty = atom<PropertySheetData | undefined>(undefined)

// The sheets render as docked side panels that take up layout width,
// so only one of them may be open at a time.
export function openRecordSheet(id: DBRecord['__id']) {
  $sheetProperty.set(undefined)
  $sheetRecordId.set(id)
}

export function openPropertySheet(property: PropertySheetData) {
  $sheetRecordId.set(undefined)
  $sheetProperty.set(property)
}
