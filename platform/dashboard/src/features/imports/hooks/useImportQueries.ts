import { useQuery } from '@tanstack/react-query'

import { rushDBInstance } from '~/lib/sdk'

import { isRunActive } from '../lib'

export const importQueryKeys = {
  runs: () => ['imports', 'runs'] as const,
  run: (runId: string) => ['imports', 'runs', runId] as const
}

export function useImportRunsQuery() {
  return useQuery({
    queryKey: importQueryKeys.runs(),
    queryFn: () => rushDBInstance.imports.list()
  })
}

export function useImportRunQuery(runId: string | undefined) {
  return useQuery({
    queryKey: importQueryKeys.run(runId ?? ''),
    queryFn: () => rushDBInstance.imports.get(runId!),
    enabled: Boolean(runId),
    refetchInterval: (query) => {
      const detail = query.state.data
      return detail && isRunActive(detail.status) ? 2000 : false
    }
  })
}
