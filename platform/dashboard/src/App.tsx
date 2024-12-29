import { useStore } from '@nanostores/react'

import { RootLayout } from '~/layout/RootLayout'
import {
  $router,
  isProjectPage,
  isProjectRecordPage,
  isProtectedRoute
} from '~/lib/router'
import { NotFoundPage } from '~/pages/404'
import { PasswordRecoveryPage } from '~/pages/forgot-password'
import { NewProjectPage } from '~/pages/project/new'
import { ProfilePage } from '~/pages/profile.tsx'
import { SignInPage } from '~/pages/signin'
import { SignUpPage } from '~/pages/signup'
import { WorkspaceBillingPage } from '~/pages/workspace/billing'
import { NewWorkspacePage } from '~/pages/workspace/new'
import { WorkspaceProjectsPage } from '~/pages/workspace/projects'
import { WorkspaceSettingsPage } from '~/pages/workspace/settings'
import { ConfirmEmail } from '~/pages/auth/confirmEmail'

import { Toaster } from './elements/Toast'
import { LiveChat } from './features/auth/components/LiveChat'
import { ProjectLayout } from './layout/ProjectLayout'
import { ProjectRecordLayout } from './layout/ProjectRecordLayout'
import { AuthGoogle } from './pages/auth/google'
import { OauthPage } from '~/pages/auth/oauth'
import { useEffect } from 'react'
import { $platformSettings } from '~/features/auth/stores/settings.ts'

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
    case 'confirmEmail':
      return <ConfirmEmail />
    case 'oauth':
      return <OauthPage />
    default:
      return <NotFoundPage />
  }
}

function ProtectedRoutes() {
  const page = useStore($router)

  if (isProjectRecordPage(page)) {
    return <ProjectRecordLayout />
  }

  if (isProjectPage(page)) {
    return <ProjectLayout />
  }

  switch (page?.route) {
    case 'profile':
      return <ProfilePage />
    case 'workspaceSettings':
      return <WorkspaceSettingsPage />
    case 'workspaceBilling':
      if ($platformSettings.get().data?.selfHosted) {
        return null
      }
      return <WorkspaceBillingPage />
    case 'projects':
    case 'home':
      return <WorkspaceProjectsPage />
    case 'newProject':
      return <NewProjectPage />
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
    const id = 'G-CLCR2SYDC6'

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
  const platformSettings = useStore($platformSettings)

  if (!import.meta.env.PROD || platformSettings.data?.selfHosted) {
    return null
  }

  return (
    <>
      <LiveChat />

      <Gtm />
    </>
  )
}

export function App() {
  return (
    <>
      <Routes />

      <Toaster />

      <ProductionScripts />
    </>
  )
}
