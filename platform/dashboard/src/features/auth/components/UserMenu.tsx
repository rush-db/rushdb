import { useStore } from '@nanostores/react'
import { LogOut, RotateCcw, User } from 'lucide-react'

import { Divider } from '~/elements/Divider'
import { IconButton } from '~/elements/IconButton'
import { Menu, MenuItem, MenuTitle } from '~/elements/Menu'
import { $user, logOut } from '~/features/auth/stores/user'
import { getRoutePath, openRoute } from '~/lib/router'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { useUpdateUserMutation } from '~/features/auth/hooks/useAuthMutations'
import { $tourRunning, setTourStep } from '~/features/tour/stores/tour.ts'
import { ThemeSwitcher } from '~/features/auth/components/ThemeSwitcher'
import { CurrentSubscriptionInfo } from '~/components/billing/CurrentSubscriptionInfo.tsx'

export function UserMenu() {
  const currentUser = useStore($user)
  const { data: platformSettings } = usePlatformSettings()
  const { mutateAsync: updateSettings } = useUpdateUserMutation()

  const handleRestart = async () => {
    await updateSettings({ settings: JSON.stringify({ onboardingStatus: 'active' }) })
    openRoute('home')
    setTourStep('welcome', true)
    $tourRunning.set(true)
  }

  return (
    <div className="flex items-center gap-2">
      <Menu
        trigger={
          <IconButton
            aria-label="User Menu"
            className="h-11 w-11 place-items-center rounded-full"
            variant="secondary"
          >
            <User />
          </IconButton>
        }
      >
        <Divider />
        {currentUser.login && (
          <MenuTitle className="truncate py-2" title={currentUser.login}>
            {currentUser.login}
          </MenuTitle>
        )}
        <Divider />
        {!platformSettings?.selfHosted ?
          <MenuItem as="a" href={getRoutePath('workspaceBilling')}>
            <CurrentSubscriptionInfo className="items-start" />
          </MenuItem>
        : null}
        <Divider />
        <MenuItem as="a" asChild href={getRoutePath('profile')} icon={<User />}>
          Profile
        </MenuItem>
        <MenuItem icon={<RotateCcw />} onClick={handleRestart}>
          Restart Onboarding
        </MenuItem>
        <Divider />
        <div className="flex items-center justify-between gap-4 px-4 py-2 text-sm font-medium">
          Theme
          <ThemeSwitcher compact />
        </div>
        <Divider />
        <MenuItem icon={<LogOut />} onClick={logOut}>
          Log Out
        </MenuItem>
      </Menu>
    </div>
  )
}
