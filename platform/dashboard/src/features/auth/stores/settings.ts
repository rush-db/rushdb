import { persistentMap } from '@nanostores/persistent'
import type { AvailableSdkLanguage } from '~/features/onboarding/types'

export type UserSettings = {
  showUnits: 'false' | 'true'
  sdkLanguage: AvailableSdkLanguage
}

export const $settings = persistentMap<UserSettings>('settings:', {
  showUnits: 'true',
  sdkLanguage: 'typescript'
})
