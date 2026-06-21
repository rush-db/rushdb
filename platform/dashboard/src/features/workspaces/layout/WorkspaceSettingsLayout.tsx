import { useStore } from '@nanostores/react'
import { Cable, SettingsIcon, Users } from 'lucide-react'

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
  }
] as const

export function WorkspaceSettingsLayout({ children }: { children: React.ReactNode }) {
  const page = useStore($router)

  return (
    <div className="flex min-h-0 flex-1">
      <aside className="bg-fill2/40 flex w-56 shrink-0 flex-col border-r px-3 py-5">
        <div className="px-3 pb-4">
          <h2 className="text-content text-base font-semibold">Workspace Settings</h2>
          <p className="text-content2 mt-1 text-sm leading-6">Manage workspace-wide configuration.</p>
        </div>
        <nav className="flex flex-col gap-1">
          {SETTINGS_ITEMS.map((item) => {
            const active = page?.route === item.route
            return (
              <a
                className={cn(
                  'text-content2 hover:bg-secondary hover:text-content flex h-9 items-center gap-3 rounded px-3 text-sm font-medium transition [&>svg]:h-4 [&>svg]:w-4',
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
