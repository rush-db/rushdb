import { LayoutDashboard, Wallet2 } from 'lucide-react'

import { PageTab, PageTabs } from '~/layout/RootLayout/PageTabs'
import { getRoutePath } from '~/lib/router'
import { cn } from '~/lib/utils'
import { $platformSettings } from '~/features/auth/stores/settings.ts'

const tabs = [
  {
    href: getRoutePath('home'),
    icon: <LayoutDashboard />,
    label: 'Projects'
  },
  !$platformSettings.get()?.data?.selfHosted
    ? {
        href: getRoutePath('workspaceBilling'),
        icon: <Wallet2 />,
        label: 'Subscription'
      }
    : undefined
]

export function WorkspacesLayout({
  children,
  className
}: TPolymorphicComponentProps<'div'>) {
  return (
    <>
      <PageTabs>
        {tabs.map(
          (tab) =>
            tab && (
              <PageTab
                href={tab.href}
                icon={tab.icon}
                label={tab.label}
                key={tab.href}
              />
            )
        )}
      </PageTabs>
      <div className={cn('flex flex-1 flex-col', className)}>{children}</div>
    </>
  )
}
