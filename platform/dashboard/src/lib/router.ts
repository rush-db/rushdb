import type { useStore } from '@nanostores/react'
import type { ParamsArg } from '@nanostores/router'

import {
  createRouter,
  createSearchParams,
  getPagePath,
  openPage,
  redirectPage
} from '@nanostores/router'
import { $platformSettings } from '~/features/auth/stores/settings.ts'

const userConfirmationLeavePublicRoutes = {
  confirmEmail: '/confirm_email'
}

const publicRoutes = {
  ...userConfirmationLeavePublicRoutes,
  passwordRecovery: '/forgot-password/:token?',
  signin: '/signin',
  signup: '/signup',
  googleAuth: '/auth/google/:token?',
  oauth: '/auth/oauth'
} as const

const projectRecordRoutes = {
  projectRecord: '/projects/:id/records/:recordId',
  projectRecordRelations: '/projects/:id/records/:recordId/relations'
} as const

const projectRoutes = {
  project: '/projects/:id',
  // projectRecords: '/projects/:id/records',
  projectSettings: '/projects/:id/settings',
  projectTokens: '/projects/:id/tokens',
  projectUsers: '/projects/:id/users',
  projectHelp: '/projects/:id/help',
  ...projectRecordRoutes
} as const

const protectedRoutes = {
  home: '/',
  newWorkspace: '/new-workspace',
  newProject: '/projects/new',
  workspaceSettings: '/workspace-settings',
  projects: '/',
  workspaceBilling: !$platformSettings.get().data?.selfHosted
    ? '/billing'
    : '/',
  profile: '/profile',
  ...projectRoutes
} as const

const routes = {
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

export const isPublicRoute = <PageName extends keyof typeof routes>(
  route?: PageName
) => Boolean(route && route in publicRoutes)

export const isProtectedRoute = <PageName extends keyof typeof routes>(
  route?: PageName
) => Boolean(route && route in protectedRoutes)

export const isUserLeaveConfirmationRoute = <
  PageName extends keyof typeof routes
>(
  route?: PageName
) => Boolean(route && route in userConfirmationLeavePublicRoutes)

export const isProjectRoute = <PageName extends keyof typeof routes>(
  route?: PageName
) => Boolean(route && route in projectRoutes)

export const isProjectRecordRoute = <PageName extends keyof typeof routes>(
  route?: PageName
) => Boolean(route && route in projectRecordRoutes)

/** check for id param existance */
export const isProjectPage = <
  Page extends ReturnType<typeof useStore<typeof $router>>
>(
  page?: Page
): page is {
  params: Record<'id', string>
  route: keyof typeof projectRoutes
} & Page => Boolean(page && page.route in projectRoutes)

export const isProjectRecordPage = <
  Page extends ReturnType<typeof useStore<typeof $router>>
>(
  page?: Page
): page is {
  params: Record<'id' | 'recordId', string>
  route: keyof typeof projectRecordRoutes
} & Page => Boolean(page && page.route in projectRecordRoutes)

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