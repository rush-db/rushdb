import { action, atom, computed } from 'nanostores'
import { persistentAtom } from '@nanostores/persistent'
import { $user } from '~/features/auth/stores/user'

export type TourStepKey =
  | 'welcome'
  | 'homeNewProject'
  | 'newProjectName'
  | 'newProjectCustomDb'
  | 'newProjectCreate'
  | 'projectSdkTokenOverview'
  | 'projectSdkTokenTabInfo'
  | 'projectImportDataTab'
  | 'projectImportOverview'
  | 'projectImportRadio'
  | 'projectImportIngest'
  | 'recordTableOverview'
  | 'recordTableSearchInput'
  | 'recordTableViewMode'

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
export const setTourStep = action(
  $tourStep,
  'setTourStep',
  (store, nextKey: TourStepKey, manual?: boolean) => {
    const idx = keys.indexOf(nextKey)
    const currIdx = keys.indexOf(store.get())
    if (!manual && idx < currIdx) return
    if (idx >= $tourMaxReached.get()) {
      $tourMaxReached.set(idx)
    }

    if (!$tourRunning.get()) {
      $tourRunning.set(true)
    }

    store.set(nextKey)
  }
)

export const $tourAllowed = computed([$user], (user) => {
  if (user.currentScope?.role !== 'owner') {
    return false
  }

  let status

  try {
    status = JSON.parse(user.settings ?? '')?.onboardingStatus as 'skipped' | 'finished' | 'active'
  } catch {}

  if (!status) {
    return true
  }

  try {
    return status === 'active'
  } catch {
    return false
  }
})

export const $tourEffective = computed([$tourRunning, $tourAllowed], (running, allowed) => running && allowed)
