import { useEffect, useRef } from 'react'
import { useStore } from '@nanostores/react'
import { $user, updateUser } from '~/features/auth/stores/user'
import { $tourRunning } from '~/features/tour/stores/tour'
import { $platformSettings } from '~/features/auth/stores/settings'

export function OnboardingInit() {
  const user = useStore($user)
  const platformSettings = useStore($platformSettings)
  const { mutate: updateSettings } = useStore(updateUser)
  const hasInitialized = useRef(false)
  const isOwner = user.currentScope?.role === 'owner'

  useEffect(() => {
    if (!user || !isOwner || !user.isLoggedIn || platformSettings.data?.selfHosted) return
    if (hasInitialized.current) return

    let settingsObj: Record<string, any> = {}
    try {
      settingsObj = typeof user.settings === 'string' ? JSON.parse(user.settings) : user.settings || {}
    } catch {
      settingsObj = {}
    }

    const status = settingsObj.onboardingStatus as 'skipped' | 'finished' | 'active' | undefined

    if (!status) {
      updateSettings({ settings: JSON.stringify({ ...settingsObj, onboardingStatus: 'active' }) })
    }

    $tourRunning.set(true)
    hasInitialized.current = true
  }, [user, platformSettings.data, updateSettings])

  return null
}
