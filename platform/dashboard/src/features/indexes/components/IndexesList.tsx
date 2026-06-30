import { Copy, Database, MoreVertical, Trash2 } from 'lucide-react'

import { Card } from '~/elements/Card'
import { ConfirmDialog } from '~/elements/ConfirmDialog'
import { IconButton } from '~/elements/IconButton'
import { Label } from '~/elements/Label'
import { Menu, MenuItem } from '~/elements/Menu'
import { NothingFound } from '~/elements/NothingFound'
import { Skeleton } from '~/elements/Skeleton'
import { toast } from '~/elements/Toast'
import { cn, copyToClipboard } from '~/lib/utils'
import { getLabelColor } from '~/features/labels'
import { useIndexStatsQuery, useProjectLabelsQuery } from '~/features/projects/hooks/useProjectQueries'

import type { EmbeddingIndex } from '../types'

import { useDeleteIndexMutation } from '../hooks/useIndexMutations'

function IndexProgressBar({ id, status }: { id: string; status: string }) {
  const isActive = status === 'pending' || status === 'indexing'
  const { data, isLoading, isFetching } = useIndexStatsQuery(id)

  if (!isActive) return null

  const totalRecords = data?.totalRecords ?? 0
  const indexedRecords = data?.indexedRecords ?? 0
  const progress =
    totalRecords > 0 ? Math.max(0, Math.min(100, Math.round((indexedRecords / totalRecords) * 100))) : 0

  return (
    <div className="mt-2 w-full">
      <span className="text-content-secondary mt-1 block text-sm">
        {isLoading && !data ?
          'Loading progress...'
        : `${indexedRecords} / ${totalRecords} records indexed (${progress}%)`}
        {isFetching && data ? ' Updating...' : null}
      </span>
      <div className="bg-border/60 h-1.5 w-full overflow-hidden rounded-full">
        <div
          className={cn('h-full rounded-full transition-all duration-500', {
            'bg-content-2': status === 'pending',
            'bg-badge-blue': status === 'indexing'
          })}
          style={{ width: `${progress}%` }}
        />
      </div>
    </div>
  )
}

const STATUS_LABELS: Record<string, string> = {
  pending: 'Pending',
  indexing: 'Indexing',
  ready: 'Ready',
  error: 'Error'
}

const STATUS_COLORS: Record<string, string> = {
  pending: 'text-content-2 border-border bg-background',
  indexing: 'text-badge-blue border-badge-blue/30 bg-badge-blue/10',
  ready: 'text-badge-green border-badge-green/30 bg-badge-green/10',
  error: 'text-badge-red border-badge-red/30 bg-badge-red/10'
}

function IndexListItem({
  className,
  id,
  label,
  propertyName,
  modelKey,
  dimensions,
  status,
  loading,
  ...props
}: TPolymorphicComponentProps<
  'li',
  ({ loading: true } & Partial<EmbeddingIndex>) | ({ loading?: false } & EmbeddingIndex)
>) {
  const { mutate } = useDeleteIndexMutation()
  const { data: labels } = useProjectLabelsQuery()
  // Reuses the same stats query the progress bar reads (deduped by react-query), so the
  // record count is available next to the status without an extra request.
  const { data: stats } = useIndexStatsQuery(id ?? '')
  const statusLabel = STATUS_LABELS[status ?? 'pending'] ?? status
  const statusColor = STATUS_COLORS[status ?? 'pending'] ?? STATUS_COLORS.pending
  const labelVariant = label ? getLabelColor(label, Object.keys(labels ?? {}).indexOf(label)) : undefined

  return (
    <li className={cn('flex items-center gap-3 px-3 py-3 sm:gap-4 sm:px-4', className)} {...props}>
      <Database size={20} />

      <div className="flex min-w-0 flex-1 flex-col">
        <span className="mb-1 flex items-center gap-2">
          <Skeleton enabled={loading}>
            <span className="flex items-center gap-1.5">
              <Label variant={labelVariant}>{label}</Label>
              <span className="text-content3">:</span>
              <span className="bg-content3/10 text-content2 w-fit rounded-sm px-1 font-mono text-xs">
                {propertyName}
              </span>
            </span>
          </Skeleton>
        </span>

        <span className="text-content-secondary font-mono text-sm font-normal">
          <Skeleton enabled={loading}>
            {modelKey && dimensions ? `${modelKey} · ${dimensions}d` : 'Loading...'}
          </Skeleton>
        </span>
        {!loading && id && status ?
          <IndexProgressBar id={id} status={status} />
        : null}
      </div>
      {!loading && stats && stats.totalRecords > 0 ?
        <span className="text-content3 text-sm tabular-nums">
          {stats.totalRecords.toLocaleString()} records
        </span>
      : null}
      <span
        className={cn('inline-flex items-center rounded-md border px-1.5 py-0.5 font-medium', statusColor)}
      >
        {statusLabel}
      </span>
      {!loading && (
        <Menu
          trigger={
            <IconButton aria-label="more" title="More" variant="ghost">
              <MoreVertical />
            </IconButton>
          }
          align="end"
        >
          <MenuItem icon={<Copy />} onClick={() => id && copyToClipboard(id, { showSuccessToast: true })}>
            Copy index ID
          </MenuItem>
          <ConfirmDialog
            handler={() => {
              if (!id) return
              toast({
                title: 'Deleting semantic index...',
                description: 'Cleanup continues in background'
              })
              mutate({ indexId: id })
              return Promise.resolve()
            }}
            trigger={
              <MenuItem dropdown icon={<Trash2 />} variant="danger">
                Delete
              </MenuItem>
            }
            description={`The semantic index for "${propertyName}" will be permanently deleted. Are you sure?`}
            title="Delete semantic index"
          />
        </Menu>
      )}
    </li>
  )
}

export function IndexesList({
  className,
  loading,
  data,
  ...props
}: TPolymorphicComponentProps<'ul', { data?: EmbeddingIndex[]; loading: boolean }>) {
  if (data && data.length < 1) {
    return <NothingFound title="No semantic indexes configured yet" />
  }

  return (
    <Card>
      <ul className={cn('flex flex-col divide-y', className)} {...props}>
        {data?.map((index) => <IndexListItem key={index.id} {...index} />)}
        {loading ?
          <>
            <IndexListItem loading />
            <IndexListItem loading />
          </>
        : null}
      </ul>
    </Card>
  )
}
