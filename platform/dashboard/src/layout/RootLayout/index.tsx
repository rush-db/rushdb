import { useStore } from '@nanostores/react'
import { ExternalLink, PanelLeftClose, PanelLeftOpen } from 'lucide-react'
import { useState } from 'react'

import { IconButton } from '~/elements/IconButton'
import { Logo } from '~/elements/Logo'
import { ConfirmEmailNotification } from '~/features/auth/components/ConfirmEmailNotification'
import { UserMenu } from '~/features/auth/components/UserMenu'

import { ChangeProjectMenu } from '~/features/projects/components/ChangeProjectMenu'
import { ProjectTabs } from '~/features/projects/components/ProjectTabs'
import { useCurrentProjectQuery } from '~/features/projects/hooks/useProjectQueries'
import { $router, isProjectPage } from '~/lib/router'
import { cn } from '~/lib/utils'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { ChangeWorkspaceMenu } from '~/features/workspaces/components/ChangeWorkspaceMenu.tsx'
import { WorkspaceTabs } from '~/features/workspaces/layout/WorkspacesLayout'
import { LimitReachedModal } from '~/components/billing/LimitReachedDialog.tsx'
import { PaymentCallbackDialog } from '~/components/billing/PaymentCallbackDialog.tsx'
import { KuLimitBanner } from '~/components/billing/KuLimitBanner'
import { KuSidebarMeter } from '~/components/billing/KuSidebarMeter'

function SidebarNav({ collapsed }: { collapsed: boolean }) {
  const page = useStore($router)
  const { data: project } = useCurrentProjectQuery()

  if (isProjectPage(page) && project) {
    return (
      <div className="flex min-h-0 flex-1 flex-col">
        <ProjectTabs collapsed={collapsed} project={project} variant="sidebar" />
      </div>
    )
  }

  return <WorkspaceTabs collapsed={collapsed} variant="sidebar" />
}

function DashboardSidebar() {
  const page = useStore($router)
  const showProjectSelector = isProjectPage(page)
  const [collapsed, setCollapsed] = useState(false)

  return (
    <aside
      className={cn(
        'bg-fill2 flex h-full shrink-0 flex-col border-r transition-[width] duration-150',
        collapsed ? 'w-16' : 'w-64'
      )}
    >
      <div
        className={cn(
          'group/sidebar-logo relative flex min-h-[72px] items-center gap-3 px-5 py-4',
          collapsed && 'justify-center px-3'
        )}
      >
        <Logo
          className={cn('shrink-0', collapsed && 'transition-opacity group-hover/sidebar-logo:opacity-0')}
          height={24}
          width={36}
        />
        {collapsed && (
          <IconButton
            aria-label="Open sidebar"
            className="bg-fill2 absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 opacity-0 shadow-lg transition-opacity group-hover/sidebar-logo:opacity-100"
            onClick={() => setCollapsed(false)}
            size="small"
            variant="secondary"
          >
            <PanelLeftOpen />
          </IconButton>
        )}
        {!collapsed &&
          (showProjectSelector ?
            <div className="min-w-0 flex-1 overflow-hidden">
              <ChangeProjectMenu />
            </div>
          : <a className="text-content pt-1 font-mono text-base" href="/">
              rushdb
            </a>)}
        {!collapsed && (
          <IconButton
            aria-label="Collapse sidebar"
            className="ml-auto shrink-0"
            onClick={() => setCollapsed(true)}
            size="small"
            variant="ghost"
          >
            <PanelLeftClose />
          </IconButton>
        )}
      </div>

      <nav
        className={cn('flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto py-3', collapsed ? 'px-2' : 'px-4')}
      >
        <SidebarNav collapsed={collapsed} />
      </nav>

      {!collapsed && (
        <a
          className="border-content/20 bg-fill hover:bg-fill2 mx-4 mb-3 flex flex-col gap-2 rounded-md border p-4 transition"
          href="https://docs.rushdb.com"
          rel="noreferrer"
          target="_blank"
        >
          <span className="text-content text-sm font-medium">Need help getting started?</span>
          <span className="text-content2 inline-flex items-center gap-2 text-sm">
            Check out our docs <ExternalLink className="h-3.5 w-3.5" />
          </span>
        </a>
      )}

      <div className={cn('flex flex-col gap-3 border-t p-4', collapsed && 'items-center px-2')}>
        {!collapsed && <KuSidebarMeter />}
        <div className={cn('flex items-center gap-2', collapsed ? 'flex-col' : 'w-full')}>
          <UserMenu />
          {!collapsed && <ChangeWorkspaceMenu />}
        </div>
      </div>
    </aside>
  )
}

function GlobalNotifications() {
  return (
    <>
      <ConfirmEmailNotification />
    </>
  )
}

function GlobalModals() {
  const { data: platformSettings } = usePlatformSettings()

  if (platformSettings?.selfHosted) {
    return null
  }
  return (
    <>
      <PaymentCallbackDialog />
      <LimitReachedModal />
    </>
  )
}

export function RootLayout({ children, className, ...props }: TPolymorphicComponentProps<'div'>) {
  return (
    <div className={cn(className, 'flex h-screen flex-col overflow-hidden')} {...props}>
      <GlobalNotifications />
      <KuLimitBanner />
      <div className="flex min-h-0 flex-1 overflow-hidden">
        <DashboardSidebar />
        <main className="flex min-w-0 flex-1 flex-col overflow-auto">{children}</main>
      </div>
      <GlobalModals />
    </div>
  )
}
