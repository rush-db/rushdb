import { action, atom, computed } from 'nanostores'
import { persistentAtom } from '@nanostores/persistent'
import { $user } from '~/features/auth/stores/user'
import type { TourStepKey } from '../types'

export const keys: TourStepKey[] = [
  'welcome',
  'homeNewProject',
  'newProjectName',
  'newProjectCustomDb',
  'newProjectCreate',
  'projectSdkTokenOverview',
  'projectSdkTokenTabInfo',
  'projectImportDataTab',
  'projectImportOverview',
  'projectImportRadio',
  'projectImportIngest',
  'recordTableOverview',
  'recordTableSearchInput',
  'recordTableViewMode'
]

export const $tourStep = persistentAtom<TourStepKey>('tour:step', 'welcome')
export const $tourRunning = atom<boolean>(false)
export const $tourMaxReached = atom<number>(0)

export const setTourStep = action($tourStep, 'setTourStep', (store, nextKey: TourStepKey, manual = false) => {
  const idx = keys.indexOf(nextKey)
  const currIdx = keys.indexOf(store.get())

  if (!manual && idx < currIdx) {
    return
  }

  if (idx >= $tourMaxReached.get()) {
    $tourMaxReached.set(idx)
  }

  if (!$tourRunning.get()) {
    $tourRunning.set(true)
  }

  store.set(nextKey)
})

export const $tourAllowed = computed([$user], (user) => {
  if (user.currentScope?.role !== 'owner') {
    return false
  }

  let status: 'skipped' | 'finished' | 'active' | undefined
  try {
    status = JSON.parse(user.settings ?? '{}')?.onboardingStatus
  } catch {
    status = undefined
  }

  if (!status) return true
  return status === 'active'
})

export const $tourEffective = computed([$tourRunning, $tourAllowed], (running, allowed) => running && allowed)
