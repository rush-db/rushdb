import { useStore } from '@nanostores/react'
import { Download } from 'lucide-react'

import { Button } from '~/elements/Button'
import { Tooltip } from '~/elements/Tooltip'
import { toast } from '~/elements/Toast'
import { useFilteredRecordsQuery } from '~/features/projects/hooks/useProjectQueries'
import { useExportRecordsMutation } from '~/features/records/hooks/useRecordMutations'
import { $recordsSearchMode } from '~/features/projects/stores/records-search'
import { downloadCsv, rowsToCsv, unwrapRow } from '~/features/records/lib/csv'

function isDbRow(record: unknown): boolean {
  const data = unwrapRow(record) as { __id?: unknown; __proptypes?: unknown }
  return Boolean(data.__id && data.__proptypes)
}

/**
 * Top-bar CSV export for the current results view.
 * - Plain records -> server-side export (respects current query + pagination).
 * - Aggregated/shaped rows (and semantic results, which the server export can't
 *   reproduce) -> client-side export of the rows currently on screen.
 * Renders nothing when there are no results.
 */
export function ExportCsvButton() {
  const { data: recordsResult } = useFilteredRecordsQuery()
  const records = recordsResult?.data
  const mode = useStore($recordsSearchMode)
  const { mutate: exportServer, isPending } = useExportRecordsMutation()

  if (!records?.length) {
    return null
  }

  const shaped = !records.every(isDbRow)
  const useClientExport = shaped || mode === 'semantic'

  const handleExport = () => {
    if (!records?.length) return

    if (useClientExport) {
      const csv = rowsToCsv(records)
      if (!csv) {
        toast({ title: 'Nothing to export' })
        return
      }
      const stamp = new Date().toISOString().replace(/[:.]/g, '-')
      downloadCsv(`export_${stamp}.csv`, csv)
      return
    }

    exportServer()
  }

  return (
    <Tooltip
      trigger={
        <Button
          aria-label="export-csv"
          disabled={isPending}
          onClick={handleExport}
          size="small"
          variant="outline"
        >
          <Download />
          Export CSV
        </Button>
      }
    >
      <div className="text-2xs text-content flex items-center gap-1 uppercase">Export current view</div>
    </Tooltip>
  )
}
