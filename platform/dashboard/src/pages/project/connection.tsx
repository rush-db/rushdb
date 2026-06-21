import { ArrowLeft, Cable, Pause, Play, RotateCcw, TestTube2 } from 'lucide-react'
import { useStore } from '@nanostores/react'

import type { Project } from '~/features/projects/types'

import { Button } from '~/elements/Button'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { Spinner } from '~/elements/Spinner'
import {
  useConnectorActionMutation,
  useConnectorEventsQuery,
  useProjectConnectorQuery
} from '~/features/connectors/hooks'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { $router, getRoutePath } from '~/lib/router'

function formatValue(value: unknown) {
  if (value === null || value === undefined || value === '') return '-'
  if (typeof value === 'object') return JSON.stringify(value, null, 2)
  return String(value)
}

function formatDate(value?: string) {
  if (!value) return '-'
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short'
  }).format(new Date(value))
}

function StatCell({ label, value }: { label: string; value: unknown }) {
  return (
    <div className="rounded-md border p-4">
      <p className="text-content2 text-xs uppercase">{label}</p>
      <p className="mt-2 truncate text-lg font-semibold">{formatValue(value)}</p>
    </div>
  )
}

export function ProjectConnection({ projectId }: { projectId: Project['id'] }) {
  const page = useStore($router)
  const { data: platformSettings, isPending: settingsPending } = usePlatformSettings()
  const connectionId = page?.route === 'projectConnection' ? page.params.connectionId : undefined
  const { data: connector, isPending } = useProjectConnectorQuery(connectionId)
  const { data: events = [], isPending: eventsPending } = useConnectorEventsQuery(connectionId)
  const { mutateAsync: runAction, isPending: actionPending } = useConnectorActionMutation()

  if (!settingsPending && !platformSettings?.synxEnabled) {
    return (
      <>
        <PageHeader contained>
          <PageTitle>Continuous sync is not configured</PageTitle>
        </PageHeader>
        <PageContent contained>
          <Button as="a" href={getRoutePath('projectImportData', { id: projectId })} variant="outline">
            <ArrowLeft size={16} />
            Back to import
          </Button>
        </PageContent>
      </>
    )
  }

  if (isPending) {
    return (
      <div className="grid flex-1 place-items-center">
        <Spinner />
      </div>
    )
  }

  if (!connector) {
    return (
      <>
        <PageHeader contained>
          <PageTitle>Connection not found</PageTitle>
        </PageHeader>
        <PageContent contained>
          <Button as="a" href={getRoutePath('projectImportData', { id: projectId })} variant="outline">
            <ArrowLeft size={16} />
            Back to import
          </Button>
        </PageContent>
      </>
    )
  }

  const stats = connector.stats ?? {}
  const config = connector.config ?? {}
  const configEntries = Object.entries(config).filter(([key]) => !['password', 'secret'].includes(key))
  const statEntries = Object.entries(stats)

  return (
    <>
      <PageHeader contained>
        <div className="flex min-w-0 items-center gap-3">
          <Cable />
          <div className="min-w-0">
            <PageTitle className="truncate">{connector.name}</PageTitle>
            <p className="text-content2 mt-2 text-sm">
              {connector.type} connection · {connector.status}
            </p>
          </div>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button
            as="a"
            href={getRoutePath('projectImportData', { id: projectId })}
            size="small"
            variant="outline"
          >
            <ArrowLeft size={16} />
            Import data
          </Button>
          <Button
            disabled={actionPending}
            onClick={() => runAction({ id: connector.id, action: 'test' })}
            size="small"
            variant="outline"
          >
            <TestTube2 size={16} />
            Test
          </Button>
          <Button
            disabled={actionPending}
            onClick={() =>
              runAction({ id: connector.id, action: connector.status === 'running' ? 'pause' : 'resume' })
            }
            size="small"
            variant="outline"
          >
            {connector.status === 'running' ?
              <Pause size={16} />
            : <Play size={16} />}
            {connector.status === 'running' ? 'Pause' : 'Resume'}
          </Button>
          <Button
            disabled={actionPending}
            onClick={() => runAction({ id: connector.id, action: 'resnapshot' })}
            size="small"
            variant="outline"
          >
            <RotateCcw size={16} />
            Resnapshot
          </Button>
        </div>
      </PageHeader>

      <PageContent contained className="gap-6">
        <section className="grid grid-cols-1 gap-4 md:grid-cols-4">
          <StatCell label="Status" value={connector.status} />
          <StatCell label="Lag" value={connector.lagMs ? `${connector.lagMs} ms` : '-'} />
          <StatCell label="Created" value={formatDate(connector.createdAt)} />
          <StatCell label="Updated" value={formatDate(connector.updatedAt)} />
        </section>

        {connector.lastError && (
          <section className="border-danger/40 bg-danger/10 rounded-md border p-4">
            <p className="text-sm font-semibold">Last error</p>
            <p className="text-content2 mt-2 text-sm">{connector.lastError}</p>
          </section>
        )}

        <section className="grid grid-cols-1 gap-6 lg:grid-cols-2">
          <div className="rounded-md border">
            <div className="border-b p-4">
              <h2 className="font-semibold">Stats</h2>
            </div>
            <div className="divide-y">
              {statEntries.length ?
                statEntries.map(([key, value]) => (
                  <div className="grid grid-cols-[160px_1fr] gap-4 p-3 text-sm" key={key}>
                    <span className="text-content2">{key}</span>
                    <span className="min-w-0 whitespace-pre-wrap break-words">{formatValue(value)}</span>
                  </div>
                ))
              : <p className="text-content2 p-4 text-sm">No sync stats yet.</p>}
            </div>
          </div>

          <div className="rounded-md border">
            <div className="border-b p-4">
              <h2 className="font-semibold">Configuration</h2>
            </div>
            <div className="divide-y">
              {configEntries.length ?
                configEntries.map(([key, value]) => (
                  <div className="grid grid-cols-[160px_1fr] gap-4 p-3 text-sm" key={key}>
                    <span className="text-content2">{key}</span>
                    <span className="min-w-0 whitespace-pre-wrap break-words">{formatValue(value)}</span>
                  </div>
                ))
              : <p className="text-content2 p-4 text-sm">No visible configuration fields.</p>}
            </div>
          </div>
        </section>

        <section className="rounded-md border">
          <div className="border-b p-4">
            <h2 className="font-semibold">Recent events</h2>
          </div>
          <div className="divide-y">
            {eventsPending ?
              <div className="grid place-items-center p-8">
                <Spinner />
              </div>
            : events.length ?
              events.map((event) => (
                <div
                  className="grid grid-cols-1 gap-2 p-4 text-sm lg:grid-cols-[180px_100px_160px_1fr]"
                  key={event.id}
                >
                  <span className="text-content2">{formatDate(event.createdAt)}</span>
                  <span className="font-medium">{event.level}</span>
                  <span>{event.type}</span>
                  <div className="min-w-0">
                    <p className="break-words">{event.message}</p>
                    {event.metadata && (
                      <pre className="text-content2 mt-2 overflow-auto whitespace-pre-wrap rounded-sm border p-2 text-xs">
                        {formatValue(event.metadata)}
                      </pre>
                    )}
                  </div>
                </div>
              ))
            : <p className="text-content2 p-4 text-sm">No events have been recorded for this connection.</p>}
          </div>
        </section>
      </PageContent>
    </>
  )
}
