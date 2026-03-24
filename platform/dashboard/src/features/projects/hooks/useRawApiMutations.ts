import { useMutation } from '@tanstack/react-query'
import { useStore } from '@nanostores/react'

import type { SearchQuery } from '@rushdb/javascript-sdk'

import { api } from '~/lib/api'
import { toast } from '~/elements/Toast'
import { $selectedOperation } from '~/features/projects/stores/raw-api'

export const useRawRecordsMutation = () => {
  const selectedOperation = useStore($selectedOperation)
  return useMutation({
    mutationFn: async ({ searchQuery }: { searchQuery: SearchQuery }) => {
      const operation = selectedOperation.split('.')
      // @ts-ignore
      return await (api?.[operation?.[0]]?.[operation?.[1]] ?? api.records.find)(searchQuery, {})
    },
    onError(error: unknown) {
      // @ts-ignore
      if (error?.message) {
        toast({
          // @ts-ignore
          title: error?.name as string,
          // @ts-ignore
          description: error?.message as string
        })
      }
    }
  })
}

export const useRawLabelsMutation = () => {
  return useMutation({
    mutationFn: ({ searchQuery }: { searchQuery: SearchQuery }) => api.labels.find({ searchQuery }),
    onError(error: unknown) {
      console.log({ error })
    }
  })
}

export const useRawPropertiesMutation = () => {
  return useMutation({
    mutationFn: ({ searchQuery }: { searchQuery: SearchQuery }) =>
      api.properties.find({ searchQuery, init: {} as RequestInit }),
    onError(error: unknown) {
      console.log({ error })
    }
  })
}
