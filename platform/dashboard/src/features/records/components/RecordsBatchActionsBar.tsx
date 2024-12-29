import { useStore } from '@nanostores/react'
import { Download, Trash } from 'lucide-react'

import { Button } from '~/elements/Button'
import { ConfirmDialog } from '~/elements/ConfirmDialog'
import { Divider } from '~/elements/Divider'
import { $exportCsv } from '~/features/projects/stores/current-project'

import {
  $hasRecordsSelection,
  $selectionLength,
  batchDeleteSelected
} from '../stores/actionbar'
import {
  $hasRelatedRecordsSelection,
  $selectionRelatedLength,
  batchDeleteRelatedSelected
} from '~/features/records/stores/related-actionbar.ts'

function DeleteSelected({ view = 'main' }: { view?: 'related' | 'main' }) {
  const { mutate, loading } = useStore(
    view === 'main' ? batchDeleteSelected : batchDeleteRelatedSelected
  )
  return (
    <ConfirmDialog
      trigger={
        <Button
          className="rounded-none text-danger hover:bg-danger/20"
          size="small"
          variant="inverse"
        >
          <Trash />
          Delete
        </Button>
      }
      handler={() => mutate({})}
      loading={loading}
      title="Delete selected records?"
    />
  )
}

function ExportRecords({ view = 'main' }: { view?: 'related' | 'main' }) {
  // @TODO: Related records export
  const { loading: exportInProgress, mutate } = useStore($exportCsv)

  return (
    <Button
      className="rounded-none text-green-700 hover:bg-danger/20"
      disabled={exportInProgress}
      onClick={mutate}
      size="small"
      variant="inverse"
    >
      <Download />
      Export csv
    </Button>
  )
}

export function RecordsBatchActionsBar({
  view = 'main'
}: {
  view?: 'related' | 'main'
}) {
  const hasSelection = useStore($hasRecordsSelection)
  const selectionLength = useStore($selectionLength)

  const hasRelatedSelection = useStore($hasRelatedRecordsSelection)
  const selectionRelatedLength = useStore($selectionRelatedLength)

  const hidden = view === 'main' ? !hasSelection : !hasRelatedSelection

  if (hidden) {
    return null
  }

  return (
    <div className="fixed bottom-24 left-1/2 z-10 flex -translate-x-1/2 items-center overflow-clip rounded-2xl  border-content/30 bg-content text-fill shadow-2xl ring animate-in">
      <span className="px-3 font-mono text-sm font-medium text-fill/70">
        {selectionLength || selectionRelatedLength} selected
      </span>
      <Divider className="bg-fill2/30" vertical />
      <DeleteSelected view={view} />
      <Divider className="bg-fill2/30" vertical />
      <ExportRecords view={view} />
    </div>
  )
}
