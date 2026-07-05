import { useStore } from '@nanostores/react'
import { Activity, Cable, KeyRound, SettingsIcon, Users } from 'lucide-react'

import { useCanUseSso } from '~/features/sso/hooks'
import { $router, getRoutePath } from '~/lib/router'
import { cn } from '~/lib/utils'

const SETTINGS_ITEMS = [
  {
    href: getRoutePath('workspaceSettings'),
    icon: <SettingsIcon />,
    label: 'General',
    route: 'workspaceSettings'
  },
  {
    href: getRoutePath('workspaceUsers'),
    icon: <Users />,
    label: 'Team Members',
    route: 'workspaceUsers'
  },
  {
    href: getRoutePath('workspaceConnectedApps'),
    icon: <Cable />,
    label: 'Connected Apps',
    route: 'workspaceConnectedApps'
  },
  {
    href: getRoutePath('workspaceSso'),
    icon: <KeyRound />,
    label: 'Single Sign-On',
    route: 'workspaceSso'
  },
  {
    href: getRoutePath('workspaceSystemInfo'),
    icon: <Activity />,
    label: 'System Info',
    route: 'workspaceSystemInfo'
  }
] as const

export function WorkspaceSettingsLayout({ children }: { children: React.ReactNode }) {
  const page = useStore($router)
  const canUseSso = useCanUseSso()
  const items = SETTINGS_ITEMS.filter((item) => item.route !== 'workspaceSso' || canUseSso)

  return (
    <div className="flex min-h-0 flex-1">
      <aside className="flex w-56 shrink-0 flex-col border-r bg-fill2/40 px-3 py-5">
        <div className="px-3 pb-4">
          <h2 className="text-base font-semibold text-content">Workspace Settings</h2>
          <p className="mt-1 text-sm leading-6 text-content2">Manage workspace-wide configuration.</p>
        </div>
        <nav className="flex flex-col gap-1">
          {items.map((item) => {
            const active = page?.route === item.route
            return (
              <a
                className={cn(
                  'flex h-9 items-center gap-3 rounded-md px-3 text-sm font-medium text-content2 transition hover:bg-secondary hover:text-content [&>svg]:h-4 [&>svg]:w-4',
                  active && 'bg-secondary text-content'
                )}
                href={item.href}
                key={item.route}
              >
                {item.icon}
                {item.label}
              </a>
            )
          })}
        </nav>
      </aside>
      <div className="min-w-0 flex-1 overflow-auto">{children}</div>
    </div>
  )
}
