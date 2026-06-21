import { Activity, LayoutDashboard, SettingsIcon, Wallet2 } from 'lucide-react'

import { PageTab, PageTabs } from '~/layout/RootLayout/PageTabs'
import { getRoutePath } from '~/lib/router'
import { cn } from '~/lib/utils'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { useStore } from '@nanostores/react'
import { useMemo } from 'react'
import { $user } from '~/features/auth/stores/user.ts'

export function WorkspaceTabs({
  collapsed = false,
  variant = 'top'
}: {
  collapsed?: boolean
  variant?: 'top' | 'sidebar'
}) {
  const currentUser = useStore($user)
  const isOwner = currentUser.currentScope?.role === 'owner'

  const { data: platformSettings } = usePlatformSettings()

  const tabsToRender = useMemo(() => {
    const workspaceTabs = [
      {
        href: getRoutePath('home'),
        icon: <LayoutDashboard />,
        label: 'Projects'
      }
    ]

    if (isOwner) {
      workspaceTabs.push({
        href: getRoutePath('workspaceSettings'),
        icon: <SettingsIcon />,
        label: 'Settings'
      })
    }

    if (!platformSettings?.selfHosted && isOwner) {
      workspaceTabs.push(
        {
          href: getRoutePath('workspaceBilling'),
          icon: <Wallet2 />,
          label: 'Billing'
        },
        {
          href: getRoutePath('workspaceApiUsage'),
          icon: <Activity />,
          label: 'API Usage'
        }
      )
    }

    return workspaceTabs
  }, [isOwner, platformSettings?.selfHosted])

  return (
    <PageTabs collapsed={collapsed} variant={variant}>
      {tabsToRender.map(
        (tab) => tab && <PageTab href={tab.href} icon={tab.icon} label={tab.label} key={tab.href} />
      )}
    </PageTabs>
  )
}

export function WorkspacesLayout({ children, className }: TPolymorphicComponentProps<'div'>) {
  return <div className={cn('flex flex-1 flex-col', className)}>{children}</div>
}
