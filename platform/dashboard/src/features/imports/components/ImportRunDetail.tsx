import { useStore } from '@nanostores/react'
import { ArrowLeft, RotateCcw, Trash2, XCircle } from 'lucide-react'

import { Button } from '~/elements/Button'
import { Card, CardBody } from '~/elements/Card'
import { ConfirmDialog } from '~/elements/ConfirmDialog'
import { IconButton } from '~/elements/IconButton'
import { NothingFound } from '~/elements/NothingFound'
import { Spinner } from '~/elements/Spinner'
import { formatIsoToLocalDateTime } from '~/lib/formatters'
import { getRoutePath } from '~/lib/router'
import { $currentProjectId } from '~/features/projects/stores/id'

import { formatBytes, isRunActive, isTerminalStatus } from '../lib'
import type { ImportRunDetail, ImportRunFile } from '../types'
import {
  useCancelImportMutation,
  useDeleteImportMutation,
  useRetryImportMutation
} from '../hooks/useImportMutations'
import { useImportRunQuery } from '../hooks/useImportQueries'
import { StatusBadge, formatRunStatus, runStatusTone } from './StatusBadge'

function isWaitingForData(file: ImportRunFile): boolean {
  return Boolean(file.waitingOn && file.waitingOn.length > 0)
}

function FileRow({ file }: { file: ImportRunFile }) {
  const waiting = isWaitingForData(file)
  return (
    <li className="flex flex-col gap-2 border-b px-3 py-3 last:border-b-0 sm:px-4">
      <div className="flex items-center gap-3">
        <div className="flex min-w-0 flex-1 flex-col">
          <span className="flex min-w-0 items-center gap-2 text-sm font-semibold">
            <span className="truncate">{file.fileName}</span>
            <StatusBadge tone={fileStatusTone(file)}>
              {formatRunStatus(file.stage || file.status)}
            </StatusBadge>
          </span>
          <span className="text-xs text-content3">
            {file.role === 'links' ? 'Relationships' : `Records · ${file.rootLabel ?? '—'}`} · {file.format} ·{' '}
            {formatBytes(file.declaredSizeBytes)}
          </span>
        </div>
        <div className="flex flex-col items-end gap-1 text-xs text-content2">
          <span>{file.recordsCommitted.toLocaleString()} records</span>
          <span>{file.relationshipsCommitted.toLocaleString()} relationships</span>
        </div>
      </div>

      <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-content2">
        <span>parsed: {file.parsedUnits.toLocaleString()}</span>
        <span>committed: {file.committedUnits.toLocaleString()}</span>
        <span>skipped: {file.skippedUnits.toLocaleString()}</span>
        {file.role === 'links' && (
          <>
            <span>links resolved: {file.linksResolved.toLocaleString()}</span>
            <span>links unresolved: {file.linksUnresolved.toLocaleString()}</span>
          </>
        )}
      </div>

      {waiting && (
        <p className="text-xs text-content2">
          Waiting for data files before relationship records can be linked.
        </p>
      )}
      {file.lastErrorMessage && <p className="text-xs text-danger">{file.lastErrorMessage}</p>}
    </li>
  )
}

function fileStatusTone(
  file: ImportRunFile
): 'neutral' | 'success' | 'danger' | 'active' | 'info' | 'warning' {
  const s = file.stage || file.status
  if (/error|failed|blocked/i.test(s)) return 'danger'
  if (/complete|committed/i.test(s)) return 'success'
  if (/waiting|queued|pending/i.test(s) || isWaitingForData(file)) return 'info'
  if (/running|parsing|finaliz/i.test(s)) return 'active'
  return 'neutral'
}

function RunSummary({ detail }: { detail: ImportRunDetail }) {
  const stats: Array<{ label: string; value: string }> = [
    { label: 'Files', value: String(detail.totalFiles) },
    { label: 'Parsed units', value: detail.parsedUnits.toLocaleString() },
    { label: 'Records committed', value: detail.recordsCommitted.toLocaleString() },
    { label: 'Relationships committed', value: detail.relationshipsCommitted.toLocaleString() },
    { label: 'Failed files', value: String(detail.failedFiles) }
  ]
  return (
    <Card>
      <CardBody className="flex-row flex-wrap gap-6">
        {stats.map((s) => (
          <div key={s.label} className="flex flex-col">
            <span className="text-xs text-content2">{s.label}</span>
            <span className="text-xl font-bold text-content tabular-nums">{s.value}</span>
          </div>
        ))}
      </CardBody>
    </Card>
  )
}

function EventsList({ detail }: { detail: ImportRunDetail }) {
  const events = detail.events ?? []
  if (events.length === 0) return null
  return (
    <section className="flex flex-col gap-2">
      <h3 className="text-base font-semibold text-content">Events</h3>
      <Card>
        <ul className="flex max-h-64 flex-col divide-y overflow-y-auto">
          {events.map((event) => (
            <li key={event.id} className="flex items-start gap-3 px-3 py-2 text-sm sm:px-4">
              <span className="mt-0.5 shrink-0 text-xs text-content3">
                {formatIsoToLocalDateTime(event.createdAt)}
              </span>
              <span className="min-w-0 flex-1 text-content2">
                {event.type}
                {event.message ? ` — ${event.message}` : ''}
              </span>
            </li>
          ))}
        </ul>
      </Card>
    </section>
  )
}

export function ImportRunDetail({ runId }: { runId: string }) {
  const projectId = useStore($currentProjectId)
  const cancel = useCancelImportMutation()
  const retry = useRetryImportMutation()
  const remove = useDeleteImportMutation()
  const { data: detail, isPending, isError } = useImportRunQuery(runId)

  if (isPending) {
    return (
      <div className="grid flex-1 place-items-center">
        <Spinner />
      </div>
    )
  }

  if (isError || !detail) {
    return <NothingFound title="Import run not found" />
  }

  const active = isRunActive(detail.status)
  const terminal = isTerminalStatus(detail.status)

  return (
    <div className="container flex flex-col gap-6 py-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <IconButton
            aria-label="Back to imports"
            as="a"
            href={getRoutePath('projectImports', { id: projectId! })}
            variant="ghost"
          >
            <ArrowLeft />
          </IconButton>
          <div className="flex flex-col">
            <div className="flex items-center gap-3">
              <h2 className="text-xl font-bold text-content">{detail.name || detail.id}</h2>
              <StatusBadge tone={runStatusTone(detail.status)}>{formatRunStatus(detail.status)}</StatusBadge>
            </div>
            <span className="text-xs text-content3">
              Created {formatIsoToLocalDateTime(detail.createdAt)}
              {detail.startedAt ? ` · Started ${formatIsoToLocalDateTime(detail.startedAt)}` : ''}
              {detail.finalizedAt ? ` · Finished ${formatIsoToLocalDateTime(detail.finalizedAt)}` : ''}
            </span>
          </div>
        </div>

        <div className="flex items-center gap-2">
          {active && detail.status !== 'canceling' && (
            <Button onClick={() => cancel.mutate(runId)} variant="secondary">
              <XCircle size={16} />
              Cancel
            </Button>
          )}
          {terminal && (
            <Button onClick={() => retry.mutate(runId)} variant="secondary">
              <RotateCcw size={16} />
              Retry
            </Button>
          )}
          {terminal && (
            <ConfirmDialog
              handler={() => remove.mutateAsync(runId)}
              title="Delete import run"
              description="The import history and any staged files will be removed. Imported records stay in the database."
              trigger={
                <Button variant="dangerGhost">
                  <Trash2 size={16} />
                  Delete
                </Button>
              }
            />
          )}
        </div>
      </div>

      <RunSummary detail={detail} />

      <section className="flex flex-col gap-2">
        <h3 className="text-base font-semibold text-content">Files</h3>
        <Card>
          <ul className="flex flex-col">
            {detail.files.map((file) => (
              <FileRow key={file.id} file={file} />
            ))}
          </ul>
        </Card>
      </section>

      <EventsList detail={detail} />
    </div>
  )
}
