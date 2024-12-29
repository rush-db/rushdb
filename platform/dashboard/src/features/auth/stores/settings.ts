import { persistentMap } from '@nanostores/persistent'
import { AvailableSdkLanguage } from '~/features/onboarding/types'
import { createAsyncStore } from '~/lib/fetcher.ts'
import { api } from '~/lib/api.ts'

export type UserSettings = {
  showUnits: 'false' | 'true'
  sdkLanguage: AvailableSdkLanguage
}

export const $settings = persistentMap<UserSettings>('settings:', {
  showUnits: 'true',
  sdkLanguage: 'typescript'
})

export const $platformSettings = createAsyncStore({
  key: '$platformSettings',
  async fetcher(init) {
    return await api.settings.get({ init })
  },
  deps: []
})
