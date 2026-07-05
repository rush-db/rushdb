import { useStore } from '@nanostores/react'
import { Cable, SettingsIcon } from 'lucide-react'

import type { WithProjectID } from '~/features/projects/types'

import { $router, getRoutePath } from '~/lib/router'
import { cn } from '~/lib/utils'

const PROJECT_SETTINGS_ITEMS = [
  {
    icon: <SettingsIcon />,
    label: 'General',
    route: 'projectSettings'
  },
  {
    icon: <Cable />,
    label: 'Connected Apps',
    route: 'projectConnectedApps'
  }
] as const

export function ProjectSettingsLayout({
  children,
  projectId
}: WithProjectID & { children: React.ReactNode }) {
  const page = useStore($router)

  return (
    <div className="flex min-h-0 flex-1">
      <aside className="flex w-56 shrink-0 flex-col border-r bg-fill2/40 px-3 py-5">
        <div className="px-3 pb-4">
          <h2 className="text-base font-semibold text-content">Project Settings</h2>
          <p className="mt-1 text-sm leading-6 text-content2">Manage settings for this project.</p>
        </div>
        <nav className="flex flex-col gap-1">
          {PROJECT_SETTINGS_ITEMS.map((item) => {
            const active = page?.route === item.route
            return (
              <a
                className={cn(
                  'flex h-9 items-center gap-3 rounded-md px-3 text-sm font-medium text-content2 transition hover:bg-secondary hover:text-content [&>svg]:h-4 [&>svg]:w-4',
                  active && 'bg-secondary text-content'
                )}
                href={getRoutePath(item.route, { id: projectId })}
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
