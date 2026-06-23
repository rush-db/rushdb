import type { useStore } from '@nanostores/react'
import type { ParamsArg } from '@nanostores/router'

import { createRouter, createSearchParams, getPagePath, openPage, redirectPage } from '@nanostores/router'
// eslint-disable-next-line import/no-cycle
import { $inviteToken } from '~/features/workspaces/stores/invite'

// Public routes that an already-authenticated user should still be able to open
// (they land here from an email link), instead of being bounced to home.
const authExemptPublicRoutes = {
  confirmEmail: '/confirm_email',
  passwordRecovery: '/forgot-password/:token?'
}

const publicRoutes = {
  ...authExemptPublicRoutes,
  signin: '/signin',
  signup: '/signup',
  googleAuth: '/auth/google/:token?',
  githubAuth: '/auth/github/:token?',
  ssoAuth: '/auth/sso',
  oauthConsent: '/oauth/consent'
} as const

export const projectRoutes = {
  project: '/projects/:id',
  projectSettings: '/projects/:id/settings',
  projectConnectedApps: '/projects/:id/settings/connected-apps',
  projectImportData: '/projects/:id/import',
  projectNewConnection: '/projects/:id/connections/new/:sourceType',
  projectConnection: '/projects/:id/connections/:connectionId',
  projectTokens: '/projects/:id/tokens',
  projectIndexes: '/projects/:id/indexes',
  projectRelationships: '/projects/:id/relationships',
  projectUsers: '/projects/:id/users',
  projectHelp: '/projects/:id/help',
  projectBilling: '/projects/:id/billing'
} as const

const protectedRoutes = {
  home: '/',
  newWorkspace: '/new-workspace',
  newProject: '/projects/new',
  workspaceSettings: '/workspace-settings',
  workspaceUsers: '/workspace-users',
  workspaceConnectedApps: '/workspace-settings/connected-apps',
  workspaceSso: '/workspace-settings/sso',
  joinWorkspace: '/join-workspace',
  projects: '/',
  workspaceBilling: '/billing',
  workspaceApiUsage: '/api-usage',
  workspaceApiKeys: '/api-keys',
  profile: '/profile',
  profileSecurity: '/profile/security',
  ...projectRoutes
} as const

export const routes = {
  ...publicRoutes,
  ...protectedRoutes
} as const

export type SearchParams = Record<string, string>

export const $router = createRouter(routes)

export const $searchParams = createSearchParams()

// utils

export function changeSearchParam(key: string, value: string) {
  $searchParams.open({ ...$searchParams.get(), [key]: value })
}

export function removeSearchParam(key: string) {
  const cpy = { ...$searchParams.get() }

  delete cpy[key]

  $searchParams.open(cpy)
}

export const isPublicRoute = <PageName extends keyof typeof routes>(route?: PageName) =>
  Boolean(route && route in publicRoutes)

export const isProtectedRoute = <PageName extends keyof typeof routes>(route?: PageName) =>
  Boolean(route && route in protectedRoutes)

export const isAuthExemptRoute = <PageName extends keyof typeof routes>(route?: PageName) =>
  Boolean(route && route in authExemptPublicRoutes)

export const isProjectRoute = <PageName extends keyof typeof routes>(route?: PageName) =>
  Boolean(route && route in projectRoutes)

/** check for id param existance */
export const isProjectPage = <Page extends ReturnType<typeof useStore<typeof $router>>>(
  page?: Page
): page is {
  params: Record<'id', string>
  route: keyof typeof projectRoutes
} & Page => Boolean(page && page.route in projectRoutes)

export const getRoutePath = <PageName extends keyof typeof routes>(
  route: PageName,
  ...params: ParamsArg<typeof routes, PageName>
) => getPagePath($router, route, ...params)

export const redirectRoute = <PageName extends keyof typeof routes>(
  route: PageName,
  ...params: ParamsArg<typeof routes, PageName>
) => redirectPage($router, route, ...params)

export const openRoute = <PageName extends keyof typeof routes>(
  route: PageName,
  ...params: ParamsArg<typeof routes, PageName>
) => openPage($router, route, ...params)

$searchParams.subscribe((params) => {
  if (params.invite) {
    $inviteToken.set(params.invite)
  }
})
