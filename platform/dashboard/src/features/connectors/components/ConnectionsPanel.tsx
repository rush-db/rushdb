import { Cable, Database, ExternalLink, Lock, Pause, Play, RotateCcw, TestTube2 } from 'lucide-react'

import { Button } from '~/elements/Button'
import { DeleteConnectorDialog } from '~/features/connectors/components/DeleteConnectorDialog'
import {
  useConnectorActionMutation,
  useConnectorCatalogQuery,
  useDeleteConnectorMutation,
  useProjectConnectorsQuery
} from '~/features/connectors/hooks'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import type { ConnectorHealth } from '~/features/connectors/types'
import { getRoutePath, openRoute } from '~/lib/router'
import { useStore } from '@nanostores/react'
import { $currentProjectId } from '~/features/projects/stores/id'

function SourceTile({
  description,
  icon,
  onClick,
  title,
  locked = false
}: {
  description: string
  icon?: string
  onClick?: () => void
  title: string
  locked?: boolean
}) {
  return (
    <button
      className={`flex w-full items-start gap-4 rounded-lg border bg-secondary px-5 py-4 text-start ring-accent-ring transition-all ${
        locked ?
          'cursor-not-allowed opacity-70 grayscale'
        : 'hover:border-accent-hover hover:bg-secondary-hover focus-visible:border-accent-focus focus-visible:ring'
      }`}
      disabled={locked}
      onClick={onClick}
    >
      <div
        className={`mt-0.5 flex size-10 shrink-0 items-center justify-center rounded-md border bg-fill3 text-accent ${
          locked ? 'grayscale' : ''
        }`}
      >
        {icon ?
          <span className="[&_svg]:size-6 [&_svg]:stroke-[1.8]" dangerouslySetInnerHTML={{ __html: icon }} />
        : <Database size={20} />}
      </div>
      <div className="min-w-0">
        <h3 className="font-bold">{title}</h3>
        <p className="text-content2">{description}</p>
      </div>
      {locked && (
        <div className="mt-1 ml-auto flex items-center gap-1 text-xs text-content2">
          <Lock size={14} />
          Upgrade
        </div>
      )}
    </button>
  )
}

function HealthBadge({ health }: { health?: ConnectorHealth }) {
  if (!health) return null
  const styles: Record<ConnectorHealth['level'], string> = {
    healthy: 'bg-emerald-500/10 text-emerald-500',
    degraded: 'bg-amber-500/10 text-amber-500',
    critical: 'bg-red-500/10 text-red-500'
  }
  return (
    <span
      className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium ${styles[health.level]}`}
      title={health.reasons.join(', ') || health.level}
    >
      {health.score}
    </span>
  )
}

export function ConnectionsPanel() {
  const projectId = useStore($currentProjectId)
  const { data: platformSettings, isPending: settingsPending } = usePlatformSettings()
  const { data: connectors = [], isPending } = useProjectConnectorsQuery()
  const { data: catalog } = useConnectorCatalogQuery()
  const { mutateAsync: runAction, isPending: actionPending } = useConnectorActionMutation()
  const { mutateAsync: deleteConnector, isPending: deletePending } = useDeleteConnectorMutation()

  if (!settingsPending && !platformSettings?.synxEnabled) {
    return null
  }

  const openSetup = (sourceType: string) => {
    if (!projectId) return
    openRoute('projectNewConnection', { id: projectId, sourceType })
  }

  const entitled = catalog?.connectors ?? []
  const locked = catalog?.unavailable ?? []

  return (
    <section className="mt-5">
      <div className="mb-3 flex flex-col gap-1 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <h2 className="flex items-center gap-2 text-base font-semibold">
            <Cable size={18} />
            Continuous sync
          </h2>
          <p className="text-sm text-content2">Connect a datasource and stream changes into this project.</p>
        </div>
        <div className="text-sm text-content2">
          {isPending ? 'Loading connections...' : `${connectors.length} configured`}
        </div>
      </div>

      <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
        {entitled.map((descriptor) => (
          <SourceTile
            key={descriptor.id}
            description={descriptor.description ?? `Stream ${descriptor.name} changes.`}
            icon={descriptor.icon}
            onClick={() => openSetup(descriptor.id)}
            title={`Connect ${descriptor.name}`}
          />
        ))}
        {locked.map((entry) => (
          <SourceTile
            key={entry.id}
            description={entry.reason}
            icon={entry.icon}
            onClick={undefined}
            title={`Connect ${entry.name}`}
            locked
          />
        ))}
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
                <p className="flex items-center gap-2 text-xs text-content2">
                  {connector.type} · {connector.status}
                  <HealthBadge health={connector.health} />
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
                <Button
                  disabled={actionPending}
                  onClick={() => runAction({ id: connector.id, action: 'start' })}
                  size="xsmall"
                  variant="outline"
                >
                  <Play size={14} />
                  Start
                </Button>
                <Button
                  disabled={actionPending}
                  onClick={() => runAction({ id: connector.id, action: 'replay' })}
                  size="xsmall"
                  variant="outline"
                >
                  <RotateCcw size={14} />
                  Replay
                </Button>
                <Button
                  disabled={actionPending}
                  onClick={() => runAction({ id: connector.id, action: 'cancel' })}
                  size="xsmall"
                  variant="outline"
                >
                  <Pause size={14} />
                  Cancel
                </Button>
                <DeleteConnectorDialog
                  connectorName={connector.name}
                  loading={deletePending}
                  onDelete={(deleteRecords) => deleteConnector({ id: connector.id, deleteRecords })}
                />
              </div>
            </div>
          ))}
        </div>
      )}
    </section>
  )
}
