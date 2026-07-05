import type { DBRecord, Property, Relation } from '@rushdb/javascript-sdk'

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

export const $sheetRelationship = atom<Relation | undefined>(undefined)

// The sheets render as docked side panels that take up layout width,
// so only one of them may be open at a time.
export function openRecordSheet(id: DBRecord['__id']) {
  $sheetProperty.set(undefined)
  $sheetRelationship.set(undefined)
  $sheetRecordId.set(id)
}

export function openPropertySheet(property: PropertySheetData) {
  $sheetRecordId.set(undefined)
  $sheetRelationship.set(undefined)
  $sheetProperty.set(property)
}

export function openRelationshipSheet(relation: Relation) {
  $sheetRecordId.set(undefined)
  $sheetProperty.set(undefined)
  $sheetRelationship.set(relation)
}
