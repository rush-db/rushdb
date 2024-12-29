import type { CollectRecord } from '@collect.so/javascript-sdk'

import { atom } from 'nanostores'

import type { Project } from '../types'

export const $currentProjectId = atom<Project['id'] | undefined>(undefined)

export const $currentRecordId = atom<CollectRecord['__id'] | undefined>(
  undefined
)

export const $sheetRecordId = atom<CollectRecord['__id'] | undefined>(undefined)
