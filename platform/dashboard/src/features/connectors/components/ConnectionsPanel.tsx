import { Cable, Database, ExternalLink, Pause, Play, RotateCcw, TestTube2 } from 'lucide-react'

import { Button } from '~/elements/Button'
import { useConnectorActionMutation, useProjectConnectorsQuery } from '~/features/connectors/hooks'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import type { ConnectorType } from '~/features/connectors/types'
import { getRoutePath, openRoute } from '~/lib/router'
import { useStore } from '@nanostores/react'
import { $currentProjectId } from '~/features/projects/stores/id'

function SourceTile({
  description,
  onClick,
  title
}: {
  description: string
  onClick: () => void
  title: string
}) {
  return (
    <button
      className="bg-secondary ring-accent-ring hover:border-accent-hover hover:bg-secondary-hover focus-visible:border-accent-focus flex w-full items-start gap-4 rounded-lg border px-5 py-4 text-start transition-all focus-visible:ring"
      onClick={onClick}
    >
      <div className="text-accent mt-1 flex items-center justify-center">
        <Database size={20} />
      </div>
      <div className="min-w-0">
        <h3 className="font-bold">{title}</h3>
        <p className="text-content2">{description}</p>
      </div>
    </button>
  )
}

export function ConnectionsPanel() {
  const projectId = useStore($currentProjectId)
  const { data: platformSettings, isPending: settingsPending } = usePlatformSettings()
  const { data: connectors = [], isPending } = useProjectConnectorsQuery()
  const { mutateAsync: runAction, isPending: actionPending } = useConnectorActionMutation()

  if (!settingsPending && !platformSettings?.synxEnabled) {
    return null
  }

  const openSetup = (sourceType: ConnectorType) => {
    if (!projectId) return
    openRoute('projectNewConnection', { id: projectId, sourceType })
  }

  return (
    <section className="mt-5">
      <div className="mb-3 flex flex-col gap-1 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <h2 className="flex items-center gap-2 text-base font-semibold">
            <Cable size={18} />
            Continuous sync
          </h2>
          <p className="text-content2 text-sm">Connect a datasource and stream changes into this project.</p>
        </div>
        <div className="text-content2 text-sm">
          {isPending ? 'Loading connections...' : `${connectors.length} configured`}
        </div>
      </div>

      <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
        <SourceTile
          description="Stream tables with logical replication."
          onClick={() => openSetup('postgres')}
          title="Connect PostgreSQL"
        />
        <SourceTile
          description="Stream collections with change streams."
          onClick={() => openSetup('mongodb')}
          title="Connect MongoDB"
        />
      </div>

      {connectors.length > 0 && (
        <div className="mt-5 divide-y rounded-md border">
          {connectors.map((connector) => (
            <div
              className="flex flex-col gap-3 p-3 sm:flex-row sm:items-center sm:justify-between"
              key={connector.id}
            >
              <div className="min-w-0">
                <p className="truncate text-sm font-semibold">{connector.name}</p>
                <p className="text-content2 text-xs">
                  {connector.type} · {connector.status}
                  {connector.lastError ? ` · ${connector.lastError}` : ''}
                </p>
              </div>
              <div className="flex flex-wrap gap-2">
                {projectId && (
                  <Button
                    as="a"
                    href={getRoutePath('projectConnection', { id: projectId, connectionId: connector.id })}
                    size="xsmall"
                    variant="outline"
                  >
                    <ExternalLink size={14} />
                    Open
                  </Button>
                )}
                <Button
                  disabled={actionPending}
                  onClick={() => runAction({ id: connector.id, action: 'test' })}
                  size="xsmall"
                  variant="outline"
                >
                  <TestTube2 size={14} />
                  Test
                </Button>
                <Button
                  disabled={actionPending}
                  onClick={() =>
                    runAction({
                      id: connector.id,
                      action: connector.status === 'running' ? 'pause' : 'resume'
                    })
                  }
                  size="xsmall"
                  variant="outline"
                >
                  {connector.status === 'running' ?
                    <Pause size={14} />
                  : <Play size={14} />}
                  {connector.status === 'running' ? 'Pause' : 'Resume'}
                </Button>
                <Button
                  disabled={actionPending}
                  onClick={() => runAction({ id: connector.id, action: 'resnapshot' })}
                  size="xsmall"
                  variant="outline"
                >
                  <RotateCcw size={14} />
                  Resnapshot
                </Button>
              </div>
            </div>
          ))}
        </div>
      )}
    </section>
  )
}
