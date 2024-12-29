import { useStore } from '@nanostores/react'
import { Copy, CurlyBraces, MoreVertical, Trash2 } from 'lucide-react'

import { Card } from '~/elements/Card'
import { ConfirmDialog } from '~/elements/ConfirmDialog'
import { IconButton } from '~/elements/IconButton'
import { Menu, MenuItem } from '~/elements/Menu'
import { NothingFound } from '~/elements/NothingFound'
import { Skeleton } from '~/elements/Skeleton'
import { $currentProjectId } from '~/features/projects/stores/id'
import { cn, copyToClipboard } from '~/lib/utils'

import type { ProjectToken } from '../types'

import { deleteToken } from '../stores/tokens'

function TokenListItem({
  className,
  description,
  expiration,
  id,
  loading,
  name,
  value,
  ...props
}: TPolymorphicComponentProps<
  'li',
  | ({ loading: true } & Partial<ProjectToken>)
  | ({ loading?: false } & ProjectToken)
>) {
  const { mutate } = useStore(deleteToken)

  return (
    <li
      className={cn(
        'flex items-center gap-3 px-3 py-3 sm:gap-4 sm:px-4',
        className
      )}
      {...props}
    >
      <CurlyBraces size={20} />

      <div className="flex min-w-0 flex-1 flex-col ">
        <span className="flex items-center gap-3 text-base font-bold">
          <Skeleton enabled={loading}>{name ?? 'Loading...'}</Skeleton>
          {description && (
            <span className="text-content-tertiary text-xs font-normal">
              {description}
            </span>
          )}
        </span>

        <span className="text-content-secondary font-mono text-xs font-normal">
          {expiration === -1 ? (
            <>Permanent</>
          ) : (
            <>
              Expires in{' '}
              <Skeleton enabled={loading}>{expiration ?? '...'}</Skeleton>
            </>
          )}
        </span>
      </div>

      <Menu
        trigger={
          <IconButton aria-label="more" title="More" variant="ghost">
            <MoreVertical />
          </IconButton>
        }
        align="end"
      >
        {value && (
          <MenuItem
            icon={<Copy />}
            onClick={() => copyToClipboard(value, { showSuccessToast: true })}
          >
            Copy token
          </MenuItem>
        )}
        <ConfirmDialog
          handler={() => {
            const projectId = $currentProjectId.get()
            if (!projectId || !id) {
              return
            }
            return mutate({ tokenId: id })
          }}
          trigger={
            <MenuItem dropdown icon={<Trash2 />} variant="danger">
              Delete
            </MenuItem>
          }
          description={`The following token will be permanently deleted, are you sure you want to continue?`}
          title={`Delete token`}
        />
      </Menu>
    </li>
  )
}

export function TokensList({
  className,
  loading,
  data,
  ...props
}: TPolymorphicComponentProps<
  'ul',
  { data?: ProjectToken[]; loading: boolean }
>) {
  if (data && data.length < 1) {
    return <NothingFound title={'No tokens exist yet'} />
  }

  return (
    <Card>
      <ul className={cn('flex flex-col divide-y', className)} {...props}>
        {data?.map((token) => (
          <TokenListItem key={token.id} {...token} />
        ))}
        {loading ? (
          <>
            <TokenListItem loading />
            <TokenListItem loading />
          </>
        ) : null}
      </ul>
    </Card>
  )
}
