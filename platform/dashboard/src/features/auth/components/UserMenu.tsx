import { useStore } from '@nanostores/react'
import { LogOut, RotateCcw, User } from 'lucide-react'

import { Divider } from '~/elements/Divider'
import { IconButton } from '~/elements/IconButton'
import { Menu, MenuItem, MenuTitle } from '~/elements/Menu'
import { $user, logOut, updateUser } from '~/features/auth/stores/user'
import { CurrentSubscriptionInfo } from '~/features/billing/components/CurrentSubscriptionInfo'
import { getRoutePath, openRoute } from '~/lib/router'
import { $platformSettings } from '~/features/auth/stores/settings.ts'
import { $tourRunning, $tourStep, setTourStep } from '~/features/tour/stores/tour.ts'

export function UserMenu() {
  const currentUser = useStore($user)
  const platformSettings = useStore($platformSettings)
  const { mutate: updateSettings } = useStore(updateUser)

  const handleRestart = async () => {
    await updateSettings({ settings: JSON.stringify({ onboardingStatus: 'active' }) })
    openRoute('home')
    setTourStep('welcome', true)
    $tourRunning.set(true)
  }

  return (
    <div className="flex items-center gap-2">
      {!platformSettings?.data?.selfHosted ?
        <a href={getRoutePath('workspaceBilling')}>
          <CurrentSubscriptionInfo />
        </a>
      : null}
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
        {!platformSettings?.data?.selfHosted ?
          <MenuItem as="a" href={getRoutePath('workspaceBilling')}>
            <CurrentSubscriptionInfo />
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
        <MenuItem icon={<LogOut />} onClick={logOut}>
          Log Out
        </MenuItem>
      </Menu>
    </div>
  )
}
