import { useStore } from '@nanostores/react'

import { $router } from '~/lib/router'
import { ImportRunDetail } from '~/features/imports/components/ImportRunDetail'

export function ProjectImportsRun() {
  const page = useStore($router)
  const runId = page?.route === 'projectImportsRun' ? page.params.runId : undefined
  if (!runId) return null
  return <ImportRunDetail runId={runId} />
}
