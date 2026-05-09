import { useEffect, useRef } from 'react'
import { useStore } from '@nanostores/react'
import { $user } from '~/features/auth/stores/user'
import { $tourRunning } from '~/features/tour/stores/tour'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { useUpdateUserMutation } from '~/features/auth/hooks/useAuthMutations'

export function OnboardingInit() {
  const user = useStore($user)
  const { data: platformSettings } = usePlatformSettings()
  const { mutateAsync: updateSettings } = useUpdateUserMutation()
  const hasInitialized = useRef(false)
  const isOwner = user.currentScope?.role === 'owner'

  useEffect(() => {
    if (!user || !isOwner || !user.isLoggedIn) return
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
  }, [user, platformSettings, updateSettings])

  return null
}
