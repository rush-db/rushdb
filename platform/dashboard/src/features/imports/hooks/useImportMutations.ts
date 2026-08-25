import { useMutation, useQueryClient } from '@tanstack/react-query'

import { toast } from '~/elements/Toast'
import { rushDBInstance } from '~/lib/sdk'

import { importQueryKeys } from './useImportQueries'

export function useCancelImportMutation() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (runId: string) => rushDBInstance.imports.cancel(runId),
    onSuccess(_, runId) {
      queryClient.invalidateQueries({ queryKey: importQueryKeys.run(runId) })
      queryClient.invalidateQueries({ queryKey: importQueryKeys.runs() })
      toast({ title: 'Import cancellation requested' })
    },
    onError() {
      toast({ title: 'Could not cancel import', variant: 'danger' })
    }
  })
}

export function useRetryImportMutation() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (runId: string) => rushDBInstance.imports.retry(runId),
    onSuccess(_, runId) {
      queryClient.invalidateQueries({ queryKey: importQueryKeys.run(runId) })
      queryClient.invalidateQueries({ queryKey: importQueryKeys.runs() })
      toast({ title: 'Import re-queued' })
    },
    onError() {
      toast({ title: 'Could not retry import', variant: 'danger' })
    }
  })
}

export function useDeleteImportMutation() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (runId: string) => rushDBInstance.imports.remove(runId),
    onSuccess() {
      queryClient.invalidateQueries({ queryKey: importQueryKeys.runs() })
      toast({ title: 'Import run deleted' })
    },
    onError() {
      toast({ title: 'Could not delete import', variant: 'danger' })
    }
  })
}
