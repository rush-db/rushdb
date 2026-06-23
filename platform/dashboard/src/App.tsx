import { useStore } from '@nanostores/react'

import { RootLayout } from '~/layout/RootLayout'
import { $router, isProjectPage, isProtectedRoute } from '~/lib/router'
import { NotFoundPage } from '~/pages/404'
import { PasswordRecoveryPage } from '~/pages/forgot-password'
import { NewProjectPage } from '~/pages/project/new'
import { NewWorkspacePage } from '~/pages/workspace/new'
import { ProfilePage, ProfileSecurityPage } from '~/pages/profile.tsx'
import { SignInPage } from '~/pages/signin'
import { SignUpPage } from '~/pages/signup'
import { WorkspaceBillingPage } from '~/pages/workspace/billing'
import { WorkspaceApiUsagePage } from '~/pages/workspace/api-usage'
import { WorkspaceApiKeysPage } from '~/pages/workspace/api-keys'
import { WorkspaceProjectsPage } from '~/pages/workspace/projects'
import { WorkspaceSettingsPage } from '~/pages/workspace/settings'
import { WorkspaceUsersPage } from '~/pages/workspace/users'
import { WorkspaceConnectedAppsPage } from '~/pages/workspace/connected-apps'
import { WorkspaceSsoPage } from '~/pages/workspace/sso-settings'
import { JoinWorkspacePage } from '~/pages/workspace/join'
import { ConfirmEmail } from '~/pages/auth/confirmEmail'
import { OAuthConsentPage } from '~/pages/oauth/consent'

import { Toaster } from './elements/Toast'
import { ProjectLayout } from './layout/ProjectLayout'
import { AuthGoogle } from './pages/auth/google'
import { AuthSso } from './pages/auth/sso'
import { useEffect, useMemo } from 'react'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { AuthGitHub } from '~/pages/auth/github.tsx'
import { $user } from '~/features/auth/stores/user.ts'
import { OnboardingTour } from '~/features/tour/components/OnboardingTour.tsx'
import { OnboardingInit } from '~/features/tour/components/OnboardingInit.tsx'
import { ErrorBoundary } from '~/features/tour/components/ErrorBoundary.tsx'

const PAGE_TITLES: Record<string, string> = {
  home: 'Projects',
  projects: 'Projects',
  newProject: 'New Project',
  newWorkspace: 'New Workspace',
  workspaceSettings: 'Workspace Settings',
  workspaceUsers: 'Workspace Members',
  workspaceConnectedApps: 'Connected Apps',
  workspaceSso: 'Single Sign-On',
  workspaceBilling: 'Billing',
  workspaceApiUsage: 'Usage Stats',
  joinWorkspace: 'Join Workspace',
  profile: 'Profile',
  profileSecurity: 'Profile Security',
  signin: 'Sign In',
  signup: 'Sign Up',
  passwordRecovery: 'Recover Account',
  confirmEmail: 'Confirm Email',
  oauthConsent: 'Authorization'
}

function useDocumentTitle() {
  const page = useStore($router)

  useEffect(() => {
    if (!page) return
    const label = PAGE_TITLES[page.route]
    if (label) {
      document.title = `${label} – RushDB`
    }
  }, [page?.route])
}

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
    case 'ssoAuth':
      return <AuthSso />
    case 'confirmEmail':
      return <ConfirmEmail />
    case 'oauthConsent':
      return <OAuthConsentPage />
    default:
      return <NotFoundPage />
  }
}

function ProtectedRoutes() {
  const page = useStore($router)
  const currentUser = useStore($user)
  const { data: platformSettings } = usePlatformSettings()

  const isOwner = useMemo(() => currentUser.currentScope?.role === 'owner', [currentUser])

  if (isProjectPage(page)) {
    return <ProjectLayout />
  }

  switch (page?.route) {
    case 'profile':
      return <ProfilePage />
    case 'profileSecurity':
      return <ProfileSecurityPage />
    case 'workspaceSettings':
      return isOwner ? <WorkspaceSettingsPage /> : null
    case 'workspaceUsers':
      return isOwner ? <WorkspaceUsersPage /> : null
    case 'workspaceConnectedApps':
      return isOwner ? <WorkspaceConnectedAppsPage /> : null
    case 'workspaceSso':
      return isOwner ? <WorkspaceSsoPage /> : null
    case 'workspaceApiKeys':
      return isOwner ? <WorkspaceApiKeysPage /> : null
    case 'joinWorkspace':
      return <JoinWorkspacePage />
    case 'workspaceBilling':
      if (platformSettings?.selfHosted || !isOwner) {
        return null
      }
      return <WorkspaceBillingPage />
    case 'workspaceApiUsage':
      if (platformSettings?.selfHosted || !isOwner) {
        return null
      }
      return <WorkspaceApiUsagePage />
    case 'projects':
    case 'home':
      return <WorkspaceProjectsPage />
    case 'newProject':
      return isOwner ? <NewProjectPage /> : null
    case 'newWorkspace':
      return <NewWorkspacePage />
    default:
      return <NotFoundPage />
  }
}

function Routes() {
  const page = useStore($router)

  useDocumentTitle()

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
    const id = import.meta.env.VITE_GTM_ID || 'GTM-XXXXX'

    // Initialise dataLayer before the GTM snippet so consent defaults
    // set via window.dataLayer.push are already present when GTM fires.
    window.dataLayer = window.dataLayer || []

    // GTM head snippet (imperative version — no <script> tags available here)
    const script = document.createElement('script')
    script.innerHTML = `(function(w,d,s,l,i){w[l]=w[l]||[];w[l].push({'gtm.start':
new Date().getTime(),event:'gtm.js'});var f=d.getElementsByTagName(s)[0],
j=d.createElement(s),dl=l!='dataLayer'?'&l='+l:'';j.async=true;j.src=
'https://www.googletagmanager.com/gtm.js?id='+i+dl;f.parentNode.insertBefore(j,f);
})(window,document,'script','dataLayer',${JSON.stringify(id)});`
    document.head.appendChild(script)

    // GTM noscript fallback
    const noscript = document.createElement('noscript')
    const iframe = document.createElement('iframe')
    iframe.src = `https://www.googletagmanager.com/ns.html?id=${id}`
    iframe.style.cssText = 'display:none;visibility:hidden'
    iframe.height = '0'
    iframe.width = '0'
    noscript.appendChild(iframe)
    document.body.insertBefore(noscript, document.body.firstChild)
  }, [])

  return null
}

function ProductionScripts() {
  const { isPending, data: platformSettings } = usePlatformSettings()

  if (isPending || platformSettings?.selfHosted) {
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
