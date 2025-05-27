import { atom } from 'nanostores'

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

export const $tourStep = atom<TourStepKey>('welcome')
export const $tourRunning = atom<boolean>(false)
