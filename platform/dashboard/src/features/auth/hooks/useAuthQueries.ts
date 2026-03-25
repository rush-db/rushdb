import { useQuery } from '@tanstack/react-query'
import { useStore } from '@nanostores/react'

import { $token } from '~/features/auth/stores/token'
import { platformSettingsQueryOptions, currentUserQueryOptions } from '../queries/authQueries'

export const usePlatformSettings = () => useQuery(platformSettingsQueryOptions)

export const useCurrentUserQuery = () => {
  const token = useStore($token)
  return useQuery(currentUserQueryOptions(token))
}
