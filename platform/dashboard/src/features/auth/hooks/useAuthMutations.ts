import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useStore } from '@nanostores/react'

import type { User } from '~/features/auth/types'

import { api } from '~/lib/api'
import { fetcher } from '~/lib/fetcher'
import { queryKeys } from '~/lib/queryKeys'
import { $user, logOut } from '~/features/auth/stores/user'

export const useUpdateUserMutation = () => {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (params: Partial<User>) => api.user.update(params),
    onSuccess(updated) {
      $user.set({ ...$user.get(), ...updated })
      queryClient.invalidateQueries({ queryKey: queryKeys.user() })
    }
  })
}

export const useDeleteUserMutation = () => {
  return useMutation({
    mutationFn: () => api.user.delete({}).then(logOut)
  })
}

export const useSendRecoveryLinkMutation = () => {
  return useMutation({
    mutationFn: ({ email }: { email?: string }) => api.auth.sendRecoveryLink({ email })
  })
}

export const useResetPasswordMutation = () => {
  return useMutation({
    mutationFn: ({ login, token, password }: { login: string; token: string; password: string }) =>
      api.auth.resetPassword({ login, token, password })
  })
}

export const useSignInMutation = () => {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ login, password }: { login?: string; password?: string }) =>
      fetcher<any>(`/api/v1/auth/login`, {
        body: JSON.stringify({ login, password }),
        method: 'POST'
      }),
    onSuccess(user) {
      $user.set({ ...user, isLoggedIn: true })
      queryClient.invalidateQueries({ queryKey: queryKeys.user() })
    }
  })
}

export const useCreateUserMutation = () => {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ login, password }: { login?: string; password?: string }) =>
      api.auth.register({ login, password }),
    onSuccess(user) {
      $user.set({ ...user, isLoggedIn: true })
      queryClient.invalidateQueries({ queryKey: queryKeys.user() })
    }
  })
}
