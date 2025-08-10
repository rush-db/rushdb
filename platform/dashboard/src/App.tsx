import { useStore } from '@nanostores/react'

import { RootLayout } from '~/layout/RootLayout'
import { $router, isProjectPage, isProtectedRoute } from '~/lib/router'
import { NotFoundPage } from '~/pages/404'
import { PasswordRecoveryPage } from '~/pages/forgot-password'
import { NewProjectPage } from '~/pages/project/new'
import { ProfilePage } from '~/pages/profile.tsx'
import { SignInPage } from '~/pages/signin'
import { SignUpPage } from '~/pages/signup'
import { WorkspaceBillingPage } from '~/pages/workspace/billing'
import { WorkspaceProjectsPage } from '~/pages/workspace/projects'
import { WorkspaceSettingsPage } from '~/pages/workspace/settings'
import { WorkspaceUsersPage } from '~/pages/workspace/users'
import { JoinWorkspacePage } from '~/pages/workspace/join'
import { ConfirmEmail } from '~/pages/auth/confirmEmail'

import { Toaster } from './elements/Toast'
import { ProjectLayout } from './layout/ProjectLayout'
import { AuthGoogle } from './pages/auth/google'
import { useEffect, useMemo } from 'react'
import { $platformSettings } from '~/features/auth/stores/settings.ts'
import { AuthGitHub } from '~/pages/auth/github.tsx'
import { $user } from '~/features/auth/stores/user.ts'
import { OnboardingTour } from '~/features/tour/components/OnboardingTour.tsx'
import { OnboardingInit } from '~/features/tour/components/OnboardingInit.tsx'
import { ErrorBoundary } from '~/features/tour/components/ErrorBoundary.tsx'
import { ProjectBillingPage } from '~/pages/project/billing.tsx'

function PublicRoutes() {
  const page = useStore($router)

  switch (page?.route) {
    case 'signin':
      return <SignInPage />
    case 'signup':
      return <SignUpPage />
    case 'passwordRecovery':
      return <PasswordRecoveryPage />
    case 'googleAuth':
      return <AuthGoogle />
    case 'githubAuth':
      return <AuthGitHub />
    case 'confirmEmail':
      return <ConfirmEmail />
    default:
      return <NotFoundPage />
  }
}

function ProtectedRoutes() {
  const page = useStore($router)
  const currentUser = useStore($user)

  const isOwner = useMemo(() => currentUser.currentScope?.role === 'owner', [currentUser])

  if (isProjectPage(page)) {
    return <ProjectLayout />
  }

  switch (page?.route) {
    case 'profile':
      return <ProfilePage />
    case 'workspaceSettings':
      return isOwner ? <WorkspaceSettingsPage /> : null
    case 'workspaceUsers':
      return isOwner ? <WorkspaceUsersPage /> : null
    case 'joinWorkspace':
      return <JoinWorkspacePage />
    case 'workspaceBilling':
      if ($platformSettings.get().data?.selfHosted || !isOwner) {
        return null
      }
      return <WorkspaceBillingPage />
    case 'projects':
    case 'home':
      return <WorkspaceProjectsPage />
    case 'newProject':
      return isOwner ? <NewProjectPage /> : null
    // case 'newWorkspace':
    //   return <NewWorkspacePage />
    default:
      return <NotFoundPage />
  }
}

function Routes() {
  const page = useStore($router)

  if (!page) {
    return <NotFoundPage />
  }

  if (isProtectedRoute(page.route)) {
    return (
      <RootLayout>
        <ProtectedRoutes />
      </RootLayout>
    )
  }

  return <PublicRoutes />
}

function Gtm() {
  useEffect(() => {
    const id = 'G-Y678D4CC1J'

    const gtagScript = document.createElement('script')
    gtagScript.src = `https://www.googletagmanager.com/gtag/js?id=${id}`
    gtagScript.async = true
    gtagScript.setAttribute('data-domain', 'app.rushdb.com')
    document.head.appendChild(gtagScript)

    // @ts-ignore
    window.dataLayer = window.dataLayer || []
    function gtag() {
      // @ts-ignore
      dataLayer.push(arguments)
    }
    // @ts-ignore
    gtag('js', new Date())
    // @ts-ignore
    gtag('config', id)
  }, [])

  return null
}

function ProductionScripts() {
  const { loading, data: platformSettings } = useStore($platformSettings)

  if (loading || platformSettings?.selfHosted) {
    return null
  }

  return <Gtm />
}

export function App() {
  return (
    <>
      <ErrorBoundary>
        <OnboardingInit />
        <OnboardingTour />
      </ErrorBoundary>
      <Routes />
      <Toaster />
      <ProductionScripts />
    </>
  )
}
