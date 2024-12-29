import { persistentMap } from '@nanostores/persistent'
import { useStore } from '@nanostores/react'
import { action, onMount, onSet, task } from 'nanostores'

import type { User } from '~/features/auth/types'

import { api } from '~/lib/api'
import { FetchError, createMutator } from '~/lib/fetcher'
import { isDeepEqual } from '~/lib/utils'

import { $token } from './token'

export const $user = persistentMap<User>(
  'user:',
  {
    isLoggedIn: false
  },
  {
    decode(value) {
      try {
        return JSON.parse(value)
      } catch {
        return value
      }
    },
    encode(value) {
      return JSON.stringify(value)
    }
  }
)

onSet($user, ({ newValue, abort }) => {
  if (isDeepEqual(newValue, $user.get())) {
    abort()
  }
})

export const logOut = action($user, 'logOut', () => {
  $user.set({ isLoggedIn: false })
  localStorage.clear()
})

export const useUser = () => useStore($user)

export const updateUser = createMutator({
  async fetcher(params: Partial<User>) {
    const newUser = await api.user.update(params)

    $user.set({ ...$user.get(), ...newUser })

    return newUser
  }
})

export const deleteUser = createMutator({
  async fetcher() {
    await api.user.delete({}).then(logOut)
  }
})

onMount($user, () => {
  task(async () => {
    try {
      if (!$token.get()) {
        return
      }

      const user = await api.user.current()

      $user.set({ ...user, isLoggedIn: true })
    } catch (error) {
      if (error instanceof FetchError) {
        if (error.response.status === 403) {
          logOut()
        }
      }
    }
  })
})
