import { useStore } from '@nanostores/react'
import { Database, MoreVertical, Trash2 } from 'lucide-react'

import { Card } from '~/elements/Card'
import { ConfirmDialog } from '~/elements/ConfirmDialog'
import { IconButton } from '~/elements/IconButton'
import { Menu, MenuItem } from '~/elements/Menu'
import { NothingFound } from '~/elements/NothingFound'
import { Skeleton } from '~/elements/Skeleton'
import { $currentProjectId } from '~/features/projects/stores/id'
import { cn } from '~/lib/utils'

import type { EmbeddingIndex } from '../types'

import { deleteIndex } from '../stores/indexes'

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
  const { mutate } = useStore(deleteIndex)
  const statusLabel = STATUS_LABELS[status ?? 'pending'] ?? status
  const statusColor = STATUS_COLORS[status ?? 'pending'] ?? STATUS_COLORS.pending

  return (
    <li className={cn('flex items-center gap-3 px-3 py-3 sm:gap-4 sm:px-4', className)} {...props}>
      <Database size={20} />

      <div className="flex min-w-0 flex-1 flex-col">
        <span className="flex items-center gap-3 text-base font-bold">
          <Skeleton enabled={loading}>{propertyName ?? 'Loading...'}</Skeleton>
          <span
            className={cn(
              'inline-flex items-center rounded-full border px-1.5 py-0.5 text-[10px] font-medium',
              statusColor
            )}
          >
            {statusLabel}
          </span>
        </span>
        <span className="text-content-secondary font-mono text-xs font-normal">
          <Skeleton enabled={loading}>
            {modelKey && dimensions ? `${modelKey} · ${dimensions}d` : 'Loading...'}
          </Skeleton>
        </span>
      </div>

      {!loading && (
        <Menu
          trigger={
            <IconButton aria-label="more" title="More" variant="ghost">
              <MoreVertical />
            </IconButton>
          }
          align="end"
        >
          <ConfirmDialog
            handler={() => {
              const projectId = $currentProjectId.get()
              if (!projectId || !id) {
                return
              }
              return mutate({ indexId: id })
            }}
            trigger={
              <MenuItem dropdown icon={<Trash2 />} variant="danger">
                Delete
              </MenuItem>
            }
            description={`The embedding index for "${propertyName}" will be permanently deleted. Are you sure?`}
            title="Delete embedding index"
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
    return <NothingFound title="No embedding indexes configured yet" />
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
