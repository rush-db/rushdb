import { Copy, KeyIcon, MoreVertical, Trash2 } from 'lucide-react'

import { Badge } from '~/elements/Badge'
import { Card } from '~/elements/Card'
import { ConfirmDialog } from '~/elements/ConfirmDialog'
import { IconButton } from '~/elements/IconButton'
import { Menu, MenuItem } from '~/elements/Menu'
import { NothingFound } from '~/elements/NothingFound'
import { Skeleton } from '~/elements/Skeleton'
import { formatIsoToLocalDateTime } from '~/lib/formatters'
import { copyToClipboard } from '~/lib/utils'

import type { WorkspaceToken } from '../types'

import { useDeleteWorkspaceTokenMutation } from '../hooks/useTokenMutations'

// `expiration` is the lifetime in ms from `created` (-1 means it never expires),
// so the absolute expiry is created + expiration.
function formatExpiry(token: WorkspaceToken): { label: string; title?: string } {
  if (token.expiration === -1) {
    return { label: 'Permanent' }
  }
  const expiresAt = new Date(token.created).getTime() + Number(token.expiration)
  const iso = new Date(expiresAt).toISOString()
  const formatted = formatIsoToLocalDateTime(iso)
  return {
    label: expiresAt < Date.now() ? `Expired ${formatted}` : `Expires ${formatted}`,
    title: iso
  }
}

function WorkspaceTokenListItem({ loading, token }: { loading?: boolean; token?: WorkspaceToken }) {
  const { mutateAsync: mutate } = useDeleteWorkspaceTokenMutation()
  const expiry = token ? formatExpiry(token) : undefined

  return (
    <li className="flex items-center gap-3 px-3 py-3 sm:gap-4 sm:px-4">
      <KeyIcon size={20} />

      <div className="flex min-w-0 flex-1 flex-col">
        <span className="flex items-center gap-3 text-base font-bold">
          <Skeleton enabled={loading}>{token?.name ?? 'Loading...'}</Skeleton>
          {token?.level === 'read' ?
            <Badge className="shrink-0">Read-only</Badge>
          : null}
        </span>
        {token?.description ?
          <span className="text-sm font-normal text-content2">{token.description}</span>
        : null}
      </div>

      <div className="flex min-w-0 flex-col items-end">
        {token?.project ?
          <Badge className="shrink-0 text-sm!">{token.project.name}</Badge>
        : null}

        <span className="text-content-secondary shrink-0 font-mono text-sm font-normal" title={expiry?.title}>
          <Skeleton enabled={loading}>{expiry?.label ?? 'Loading...'}</Skeleton>
        </span>
      </div>
      <Menu
        align="end"
        trigger={
          <IconButton aria-label="more" title="More" variant="ghost">
            <MoreVertical />
          </IconButton>
        }
      >
        {token?.value ?
          <MenuItem icon={<Copy />} onClick={() => copyToClipboard(token.value!, { showSuccessToast: true })}>
            Copy token
          </MenuItem>
        : null}
        <ConfirmDialog
          handler={() => {
            if (!token) return
            return mutate({ projectId: token.project.id, tokenId: token.id })
          }}
          trigger={
            <MenuItem dropdown icon={<Trash2 />} variant="danger">
              Delete
            </MenuItem>
          }
          description="The following token will be permanently deleted, are you sure you want to continue?"
          title="Delete token"
        />
      </Menu>
    </li>
  )
}

export function WorkspaceTokensList({ data, loading }: { data?: WorkspaceToken[]; loading: boolean }) {
  if (!loading && data && data.length < 1) {
    return <NothingFound title="No API keys created yet" />
  }

  return (
    <Card>
      <ul className="flex flex-col divide-y">
        {data?.map((token) => <WorkspaceTokenListItem key={token.id} token={token} />)}
        {loading ?
          <>
            <WorkspaceTokenListItem loading />
          </>
        : null}
      </ul>
    </Card>
  )
}
