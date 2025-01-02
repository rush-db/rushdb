import { action } from 'nanostores'

import type { GetUserResponse } from '~/features/auth/types'
import type { ApiParams } from '~/lib/api'
import type { SearchParams } from '~/lib/router'

import { api } from '~/lib/api'
import { createMutator, fetcher } from '~/lib/fetcher'
import {
  $router,
  isProtectedRoute,
  isPublicRoute,
  isUserLeaveConfirmationRoute,
  redirectRoute
} from '~/lib/router'

import { $token } from './token'
import { $user } from './user'

$user.subscribe(({ isLoggedIn, token }, changedKey) => {
  if (changedKey === 'isLoggedIn') {
    const page = $router.get()

    if (isLoggedIn) {
      $token.set(token)
    } else {
      $token.set(undefined)
    }

    if (isUserLeaveConfirmationRoute(page?.route)) {
      return null
    }

    if (!isProtectedRoute(page?.route) && isLoggedIn === true) {
      redirectRoute('home')
    }

    if (isLoggedIn === false && isProtectedRoute(page?.route)) {
      redirectRoute('signin')
    }
  }
})

$router.subscribe((page) => {
  if ($user) {
    const { isLoggedIn } = $user.get()

    if (isUserLeaveConfirmationRoute(page?.route)) {
      return null
    }

    if (isProtectedRoute(page?.route) && !isLoggedIn) {
      redirectRoute('signin')
    } else if (isLoggedIn && isPublicRoute(page?.route)) {
      redirectRoute('home')
    }
  }
})

export const logIn = action(
  $user,
  'logIn',
  (store, { login, password }: { login?: string; password?: string }) => {
    return fetcher<GetUserResponse['data']>(`/api/v1/auth/login`, {
      body: JSON.stringify({ login, password }),
      method: 'POST'
    }).then((user) => {
      store.set({ ...user, isLoggedIn: true })
    })
  }
)

export const logInGoogle = action($user, 'logInGoogle', (store, searchParams: SearchParams) => {
  const query = new URLSearchParams(searchParams).toString()
  return fetcher<GetUserResponse['data']>(`/api/v1/auth/google/callback?${query}`).then((user) => {
    $user.set({ ...user, isLoggedIn: true })
  })
})

export const oauthLogin = action($user, 'oauthLogin', (store, token: string) => {
  if (token) {
    $token.set(token)

    api.user
      .current()
      .then((user) => {
        $user.set({ ...user, isLoggedIn: true })
      })
      .catch(() => {
        $token.set(undefined)
      })
  }
})

export const confirmEmail = action($user, 'confirmEmail', (store, searchParams: SearchParams) => {
  const query = new URLSearchParams(searchParams).toString()

  return fetcher<GetUserResponse['data']>(`/api/v1/auth/confirm?${query}`).then((user) => {
    store.set({ ...user, isLoggedIn: true })
  })
})

export const createUser = action($user, 'createUser', (store, params: ApiParams<typeof api.auth.register>) =>
  api.auth.register(params).then((user) => {
    store.set({ ...user, isLoggedIn: true })
  })
)

export const $sendRecoveryLink = createMutator({
  async fetcher({ init, email }: ApiParams<typeof api.auth.sendRecoveryLink>) {
    return await api.auth.sendRecoveryLink({ init, email })
  }
})

export const resetPassword = (body: ApiParams<typeof api.auth.resetPassword>) =>
  api.auth.resetPassword(body).then((user) => {
    $user.set({ ...user, isLoggedIn: true })
  })

export const resendConfirmationLink = () => {
  const { id } = $user.get()

  if (!id) {
    throw new Error('no user id')
  }

  return api.auth.resendConfirmationLink(id)
}
