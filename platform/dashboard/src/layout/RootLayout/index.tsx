import { useStore } from '@nanostores/react'

import { AngledSeparator } from '~/elements/Divider'
import { Logo } from '~/elements/Logo'
import { ConfirmEmailNotification } from '~/features/auth/components/ConfirmEmailNotification'
import { UserMenu } from '~/features/auth/components/UserMenu'
import { LimitReachedModal } from '~/features/billing/components/LimitReachedDialog'
import { PaymentCallbackDialog } from '~/features/billing/components/PaymentCallbackDialog'
import { ChangeProjectMenu } from '~/features/projects/components/ChangeProjectMenu'
import { $currentRecord } from '~/features/projects/stores/current-record'
import { $currentProjectId } from '~/features/projects/stores/id'
import { RecordTitle } from '~/features/records/components/RecordTitle'
import { $router, isProjectPage } from '~/lib/router'
import { cn } from '~/lib/utils'
import { $platformSettings } from '~/features/auth/stores/settings.ts'

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

function CurrentRecordTitle() {
  const { data: record } = useStore($currentRecord)

  if (!record) {
    return null
  }

  return (
    <>
      <AngledSeparator />
      <RecordTitle className="px-3" id={record.__id} label={record.__label} />
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
          {/* <ChangeWorkspaceMenu /> */}

          {/* {isProjectPage(page) && (
            <>
              <AngledSeparator />
              <Button
                as="a"
                href={getRoutePath('projects')}
                size="small"
                variant="link"
              >
                Projects
              </Button>
            </>
          )} */}

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
  const platformSettings = useStore($platformSettings)

  if (platformSettings.data?.selfHosted) {
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
      <Header />
      <main className="flex flex-1 flex-col">{children}</main>
      <GlobalModals />
    </div>
  )
}
