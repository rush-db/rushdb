import { LayoutDashboard, Wallet2 } from 'lucide-react'

import { PageTab, PageTabs } from '~/layout/RootLayout/PageTabs'
import { getRoutePath } from '~/lib/router'
import { cn } from '~/lib/utils'
import { $platformSettings } from '~/features/auth/stores/settings.ts'
import { useStore } from '@nanostores/react'
import { useEffect, useState } from 'react'

export function WorkspacesLayout({ children, className }: TPolymorphicComponentProps<'div'>) {
  const [tabs, setTabs] = useState([
    {
      href: getRoutePath('home'),
      icon: <LayoutDashboard />,
      label: 'Projects'
    }
  ])

  const platformSettings = useStore($platformSettings)

  useEffect(() => {
    if (!platformSettings.loading && !platformSettings.data?.selfHosted) {
      setTabs([
        ...tabs,
        {
          href: getRoutePath('workspaceBilling'),
          icon: <Wallet2 />,
          label: 'Subscription'
        }
      ])
    }
  }, [platformSettings])

  return (
    <>
      <PageTabs>
        {tabs.map(
          (tab) => tab && <PageTab href={tab.href} icon={tab.icon} label={tab.label} key={tab.href} />
        )}
      </PageTabs>
      <div className={cn('flex flex-1 flex-col', className)}>{children}</div>
    </>
  )
}
