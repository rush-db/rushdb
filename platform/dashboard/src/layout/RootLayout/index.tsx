import { useStore } from '@nanostores/react'

import { AngledSeparator } from '~/elements/Divider'
import { Logo } from '~/elements/Logo'
import { ConfirmEmailNotification } from '~/features/auth/components/ConfirmEmailNotification'
import { UserMenu } from '~/features/auth/components/UserMenu'

import { ChangeProjectMenu } from '~/features/projects/components/ChangeProjectMenu'
import { $currentProjectId } from '~/features/projects/stores/id'
import { $router, getRoutePath, isProjectPage } from '~/lib/router'
import { cn } from '~/lib/utils'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { ChangeWorkspaceMenu } from '~/features/workspaces/components/ChangeWorkspaceMenu.tsx'
import { Button } from '~/elements/Button'
import { LimitReachedModal } from '~/components/billing/LimitReachedDialog.tsx'
import { PaymentCallbackDialog } from '~/components/billing/PaymentCallbackDialog.tsx'
import { KuHeaderBar } from '~/components/billing/KuHeaderBar.tsx'
import { KuLimitBanner } from '~/components/billing/KuLimitBanner'

function ProjectNav() {
  const page = useStore($router)
  const currentProjectId = useStore($currentProjectId)

  const projectId = isProjectPage(page) ? page?.params.id : (currentProjectId ?? undefined)

  if (!projectId) {
    return null
  }

  return (
    <>
      <AngledSeparator />
      <ChangeProjectMenu />
    </>
  )
}

function Header() {
  const page = useStore($router)

  return (
    <header className={cn('bg-fill2 flex items-center justify-between gap-2 px-2 py-2 sm:px-5')}>
      <div className="gap-inherit flex items-center">
        <Logo />

        <nav className="flex items-center overflow-auto text-sm">
          <ChangeWorkspaceMenu />

          {isProjectPage(page) && (
            <>
              <AngledSeparator />
              <Button as="a" href={getRoutePath('projects')} size="small" variant="link">
                Projects
              </Button>
            </>
          )}

          {isProjectPage(page) && <ProjectNav />}

          {page?.route === 'newProject' && (
            <>
              <AngledSeparator />
              New Project
            </>
          )}
        </nav>
      </div>

      <div className="flex items-center gap-3">
        <KuHeaderBar />
        <UserMenu />
      </div>
    </header>
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
    <div className={cn(className, 'flex min-h-screen flex-col')} {...props}>
      <GlobalNotifications />
      <KuLimitBanner />
      <Header />
      <main className="flex flex-1 flex-col">{children}</main>
      <GlobalModals />
    </div>
  )
}
