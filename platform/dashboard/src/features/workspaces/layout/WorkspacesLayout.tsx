import { LayoutDashboard, SettingsIcon, Wallet2, Users } from 'lucide-react'

import { PageTab, PageTabs } from '~/layout/RootLayout/PageTabs'
import { getRoutePath } from '~/lib/router'
import { cn } from '~/lib/utils'
import { $platformSettings } from '~/features/auth/stores/settings.ts'
import { useStore } from '@nanostores/react'
import { useMemo } from 'react'
import { $user } from '~/features/auth/stores/user.ts'

export function WorkspacesLayout({ children, className }: TPolymorphicComponentProps<'div'>) {
  const currentUser = useStore($user)
  const isOwner = currentUser.currentScope?.role === 'owner'

  const { data: platformSettings } = useStore($platformSettings)

  const tabsToRender = useMemo(() => {
    const workspaceTabs = [
      {
        href: getRoutePath('home'),
        icon: <LayoutDashboard />,
        label: 'Projects'
      }
    ]

    if (isOwner) {
      workspaceTabs.push(
        {
          href: getRoutePath('workspaceUsers'),
          icon: <Users />,
          label: 'Users'
        },
        {
          href: getRoutePath('workspaceSettings'),
          icon: <SettingsIcon />,
          label: 'Settings'
        }
      )
    }

    if (!platformSettings?.selfHosted) {
      workspaceTabs.push({
        href: getRoutePath('workspaceBilling'),
        icon: <Wallet2 />,
        label: 'Subscription'
      })
    }

    return workspaceTabs
  }, [isOwner, platformSettings?.selfHosted])

  return (
    <>
      <PageTabs>
        {tabsToRender.map(
          (tab) => tab && <PageTab href={tab.href} icon={tab.icon} label={tab.label} key={tab.href} />
        )}
      </PageTabs>
      <div className={cn('flex flex-1 flex-col', className)}>{children}</div>
    </>
  )
}
