import type { ReactNode } from 'react'

import { Eye, EyeOff, FolderOpen, Globe, KeyRound } from 'lucide-react'
import { useState } from 'react'

import { Button, CopyButton } from '~/elements/Button'
import { Card } from '~/elements/Card'
import { Skeleton } from '~/elements/Skeleton'
import { useProjectTokensQuery } from '~/features/projects/hooks/useProjectQueries'
import type { Project } from '~/features/projects/types'
import { BASE_URL } from '~/config'
import { getRoutePath } from '~/lib/router'
import { capitalize, cn } from '~/lib/utils'

// Mirrors the fetcher: an empty BASE_URL means the API is served from the same origin.
const apiUrl = () => BASE_URL || window.location.origin

const maskToken = (value: string) => `${value.slice(0, 8)}${'•'.repeat(14)}`

function ConnectionRow({
  actions,
  children,
  icon,
  label
}: {
  actions?: ReactNode
  children: ReactNode
  icon: ReactNode
  label: string
}) {
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3 sm:flex-nowrap">
      <span className="flex w-28 shrink-0 items-center gap-2 text-sm text-content2 [&>svg]:h-4 [&>svg]:w-4">
        {icon}
        {label}
      </span>
      <div className="min-w-0 flex-1 font-mono text-sm text-content">{children}</div>
      {actions && <div className="flex shrink-0 flex-wrap items-center gap-1">{actions}</div>}
    </div>
  )
}

function ProjectStatusBadge({ status = 'active' }: { status?: Project['status'] }) {
  const ready = status === 'active'

  return (
    <span
      className={cn('flex h-5 shrink-0 items-center rounded-full px-2 text-2xs font-medium', {
        'bg-badge-green/15 text-badge-green': ready,
        'bg-danger/15 text-danger': status === 'suspended',
        'bg-warning/15 text-warning': status === 'pending' || status === 'provisioning'
      })}
    >
      {ready ? 'Ready' : capitalize(status)}
    </span>
  )
}

export function ProjectConnectionPanel({ loading, project }: { loading?: boolean; project?: Project }) {
  const { data: tokens, isPending: tokensPending } = useProjectTokensQuery()
  const [revealed, setRevealed] = useState(false)

  const tokenValue = tokens?.[0]?.value
  const tokensRoute = project ? getRoutePath('projectTokens', { id: project.id }) : undefined

  return (
    <Card>
      <header className="border-b px-4 py-3 text-sm font-semibold text-content">Project connection</header>

      <div className="flex flex-col divide-y">
        <ConnectionRow
          actions={
            tokenValue ?
              <>
                <Button onClick={() => setRevealed((value) => !value)} size="xsmall" variant="ghost">
                  {revealed ?
                    <EyeOff />
                  : <Eye />}
                  {revealed ? 'Hide' : 'Reveal'}
                </Button>
                <CopyButton size="xsmall" text={tokenValue} variant="ghost">
                  Copy
                </CopyButton>
                {tokensRoute && (
                  <Button as="a" href={tokensRoute} size="xsmall" variant="outline">
                    <KeyRound />
                    Manage keys
                  </Button>
                )}
              </>
            : !tokensPending && tokensRoute ?
              <Button as="a" href={tokensRoute} size="xsmall" variant="secondary">
                <KeyRound />
                Create API key
              </Button>
            : undefined
          }
          icon={<KeyRound />}
          label="API key"
        >
          {tokensPending ?
            <Skeleton className="h-4 w-44" enabled />
          : tokenValue ?
            <span className="block truncate">{revealed ? tokenValue : maskToken(tokenValue)}</span>
          : <span className="font-sans text-content2">No API keys yet</span>}
        </ConnectionRow>

        <ConnectionRow
          actions={
            <CopyButton size="xsmall" text={apiUrl()} variant="ghost">
              Copy
            </CopyButton>
          }
          icon={<Globe />}
          label="API URL"
        >
          <span className="block truncate">{apiUrl()}</span>
        </ConnectionRow>

        <ConnectionRow icon={<FolderOpen />} label="Project">
          <Skeleton enabled={Boolean(loading)}>
            <span className="flex min-w-0 items-center gap-2">
              <span className="truncate font-sans font-medium">{project?.name ?? '...'}</span>
              {project && (
                <span className="hidden truncate text-content3 sm:block" title={project.id}>
                  {project.id.slice(0, 8)}…
                </span>
              )}
              <ProjectStatusBadge status={project?.status} />
            </span>
          </Skeleton>
        </ConnectionRow>
      </div>
    </Card>
  )
}
