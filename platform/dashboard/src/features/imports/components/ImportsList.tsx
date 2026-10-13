import { useStore } from '@nanostores/react'
import { ArrowRight, MoreVertical, RotateCcw, Trash2, XCircle } from 'lucide-react'

import { $currentProjectId } from '~/features/projects/stores/id'
import { Card } from '~/elements/Card'
import { ConfirmDialog } from '~/elements/ConfirmDialog'
import { IconButton } from '~/elements/IconButton'
import { Menu, MenuItem } from '~/elements/Menu'
import { NothingFound } from '~/elements/NothingFound'
import { Skeleton } from '~/elements/Skeleton'
import { formatIsoToLocalDateTime } from '~/lib/formatters'
import { getRoutePath } from '~/lib/router'
import { cn } from '~/lib/utils'

import {
  useCancelImportMutation,
  useDeleteImportMutation,
  useRetryImportMutation
} from '../hooks/useImportMutations'
import { formatBytes, isRunActive, isTerminalStatus } from '../lib'
import type { ImportRun } from '../types'
import { StatusBadge, formatRunStatus, runStatusTone } from './StatusBadge'

function RunRow({ run, loading }: { run?: ImportRun; loading?: boolean }) {
  const projectId = useStore($currentProjectId)
  const cancel = useCancelImportMutation()
  const retry = useRetryImportMutation()
  const remove = useDeleteImportMutation()

  const terminal = run ? isTerminalStatus(run.status) : true
  const active = run ? isRunActive(run.status) : false
  const name = run?.name || run?.id

  return (
    <li className="flex items-center gap-3 px-3 py-3 sm:gap-4 sm:px-4">
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <span className="flex min-w-0 items-center gap-3 text-base font-bold">
          <Skeleton enabled={loading} className="min-w-0">
            <span className="truncate">{name}</span>
          </Skeleton>
          {run && <StatusBadge tone={runStatusTone(run.status)}>{formatRunStatus(run.status)}</StatusBadge>}
        </span>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm font-normal text-content2">
          <Skeleton enabled={loading}>
            <span>{run ? `${run.totalFiles} file${run.totalFiles === 1 ? '' : 's'}` : ''}</span>
          </Skeleton>
          <Skeleton enabled={loading}>
            <span>{run ? formatBytes(run.totalBytes) : ''}</span>
          </Skeleton>
          <Skeleton enabled={loading}>
            <span>{run ? `${run.recordsCommitted.toLocaleString()} records` : ''}</span>
          </Skeleton>
          <Skeleton enabled={loading}>
            <span>{run ? `${run.relationshipsCommitted.toLocaleString()} relationships` : ''}</span>
          </Skeleton>
          {run && <span className="text-content3">{formatIsoToLocalDateTime(run.createdAt)}</span>}
        </div>
      </div>

      {run && (
        <Menu
          align="end"
          trigger={
            <IconButton aria-label="Import actions" title="More" variant="ghost">
              <MoreVertical />
            </IconButton>
          }
        >
          <MenuItem
            as="a"
            icon={<ArrowRight />}
            href={getRoutePath('projectImportsRun', { id: projectId!, runId: run.id })}
          >
            Open detail
          </MenuItem>
          {active && run.status !== 'canceling' && (
            <ConfirmDialog
              handler={() => cancel.mutateAsync(run.id)}
              title="Cancel import"
              description="The running import will be stopped. Already committed records are kept."
              trigger={
                <MenuItem dropdown icon={<XCircle />} variant="danger">
                  Cancel
                </MenuItem>
              }
            />
          )}
          {terminal && (
            <ConfirmDialog
              handler={() => retry.mutateAsync(run.id)}
              title="Retry import"
              description="This will re-queue the import run to process any files that did not complete."
              trigger={
                <MenuItem dropdown icon={<RotateCcw />}>
                  Retry
                </MenuItem>
              }
            />
          )}
          {terminal && (
            <ConfirmDialog
              handler={() => remove.mutateAsync(run.id)}
              title="Delete import run"
              description="The import history and any staged files will be removed. Imported records stay in the database."
              trigger={
                <MenuItem dropdown icon={<Trash2 />} variant="danger">
                  Delete
                </MenuItem>
              }
            />
          )}
        </Menu>
      )}
    </li>
  )
}

export function ImportsList({
  className,
  data,
  loading
}: {
  className?: string
  data?: ImportRun[]
  loading: boolean
}) {
  if (data && data.length < 1) {
    return <NothingFound title="No import runs yet" />
  }

  return (
    <Card>
      <ul className={cn('flex flex-col divide-y', className)}>
        {data?.map((run) => <RunRow key={run.id} run={run} />)}
        {loading ?
          <RunRow loading />
        : null}
      </ul>
    </Card>
  )
}
