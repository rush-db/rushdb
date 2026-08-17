import {
  Cable,
  Check,
  Database,
  KeyRound,
  ListFilter,
  LockKeyhole,
  RefreshCw,
  Server,
  UserRound
} from 'lucide-react'
import { useMemo, useState } from 'react'
import { useStore } from '@nanostores/react'

import { Button } from '~/elements/Button'
import { TextField } from '~/elements/Input'
import {
  discoverConnectorStreams,
  listConnectorDatabases,
  useCreateConnectorMutation,
  useUpdateConnectorMutation,
  type DiscoveredStream
} from '~/features/connectors/hooks'
import { $currentProjectId } from '~/features/projects/stores/id'
import { cn } from '~/lib/utils'

type Step = 'connection' | 'database' | 'scope' | 'rules' | 'review'

const steps: Array<{ id: Step; label: string; icon: typeof Server }> = [
  { id: 'connection', label: 'Connection', icon: Server },
  { id: 'database', label: 'Database', icon: Database },
  { id: 'scope', label: 'Scope', icon: ListFilter },
  { id: 'rules', label: 'Rules', icon: ListFilter },
  { id: 'review', label: 'Review', icon: Check }
]

const entityKey = (stream: DiscoveredStream) => stream.name

function RequiredHint() {
  return (
    <span className="inline-flex items-center gap-1 text-content2">
      <span className="text-danger">*</span>
      <span className="text-2xs">required</span>
    </span>
  )
}

function OptionalHint() {
  return <span className="text-2xs tracking-wide text-content2/70 uppercase">optional</span>
}

export function ConnectorSetupWizard({ onClose, sourceType }: { onClose: () => void; sourceType: string }) {
  const { mutateAsync: createConnector, isPending: creating } = useCreateConnectorMutation()
  const { mutateAsync: updateConnector, isPending: updating } = useUpdateConnectorMutation()
  const projectId = useStore($currentProjectId)
  // Schema-driven spec connectors (hubspot, salesforce, …) use a base URL +
  // access token instead of a host/port database connection.
  const isSpec = !['postgres', 'mysql', 'mongodb'].includes(sourceType)
  const [step, setStep] = useState<Step>('connection')
  const [connectorId, setConnectorId] = useState<string | null>(null)
  const [name, setName] = useState(
    sourceType === 'postgres' ? 'PostgreSQL source'
    : sourceType === 'mysql' ? 'MySQL source'
    : sourceType === 'mongodb' ? 'MongoDB source'
    : `${sourceType} source`
  )
  const [useUri, setUseUri] = useState(false)
  const [uri, setUri] = useState('')
  const [host, setHost] = useState('')
  const [port, setPort] = useState(
    sourceType === 'postgres' ? '5432'
    : sourceType === 'mysql' ? '3306'
    : '27017'
  )
  const [user, setUser] = useState('')
  const [password, setPassword] = useState('')
  const [baseUrl, setBaseUrl] = useState('')
  const [accessToken, setAccessToken] = useState('')
  const [database, setDatabase] = useState('')
  const [databases, setDatabases] = useState<string[]>([])
  const [listingDatabases, setListingDatabases] = useState(false)
  const [databasesError, setDatabasesError] = useState<string | null>(null)
  const [entities, setEntities] = useState<Set<string>>(new Set())
  const [ignore, setIgnore] = useState('')
  const [streams, setStreams] = useState<DiscoveredStream[]>([])
  const [discovering, setDiscovering] = useState(false)
  const [discoveryError, setDiscoveryError] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  const stepIndex = steps.findIndex((item) => item.id === step)
  const visibleSteps = isSpec ? steps.filter((s) => s.id !== 'database') : steps
  const selectedEntities = useMemo(() => [...entities], [entities])
  const ignorePatterns = useMemo(
    () =>
      ignore
        .split('\n')
        .map((value) => value.trim())
        .filter(Boolean),
    [ignore]
  )

  const isPending = creating || updating

  // Connection fields: a full URI (Mongo) counts as host+port together.
  const hostOk = useUri ? /^mongodb(\+srv)?:\/\//i.test(uri.trim()) : Boolean(host.trim())
  const canContinue =
    step === 'connection' ? Boolean(name.trim() && (isSpec ? baseUrl.trim() : hostOk))
    : step === 'database' ? Boolean(database)
    : true

  const mongoConfig = (extra: Record<string, unknown> = {}) =>
    useUri || /^mongodb(\+srv)?:\/\//i.test(host.trim()) ?
      { uri: (useUri ? uri : host).trim(), database, snapshot: true, ...extra }
    : { host, port: Number(port || 27017), database, user, snapshot: true, ...extra }

  const pgConfig = (extra: Record<string, unknown> = {}) => ({
    host,
    port: Number(port || 5432),
    database,
    user,
    snapshot: true,
    ...extra
  })

  const mysqlConfig = (extra: Record<string, unknown> = {}) => ({
    host,
    port: Number(port || 3306),
    database,
    user,
    snapshot: true,
    ...extra
  })

  const specConfig = (extra: Record<string, unknown> = {}) => ({
    baseUrl: baseUrl.trim(),
    snapshot: true,
    ...extra
  })

  const buildConfig = (extra: Record<string, unknown> = {}) => {
    if (isSpec) return specConfig(extra)
    if (sourceType === 'postgres') return pgConfig(extra)
    if (sourceType === 'mysql') return mysqlConfig(extra)
    return mongoConfig(extra)
  }

  // MySQL/Postgres name their discovered streams "tables"; Mongo "collections";
  // spec connectors use "streams".
  const scopeKey =
    isSpec ? 'streams'
    : sourceType === 'mongodb' ? 'collections'
    : 'tables'

  const ensureConnector = async (): Promise<string> => {
    if (connectorId) {
      return connectorId
    }
    const connector = await createConnector({
      name: name.trim(),
      type: sourceType,
      config: buildConfig(),
      secrets: isSpec ? { accessToken } : { password },
      transform: { naming: 'preserve', mergeStrategy: 'append' }
    })
    setConnectorId(connector.id)
    return connector.id
  }

  const listDatabases = async (id?: string) => {
    const target = id ?? connectorId
    if (!target || !projectId) {
      return
    }
    setListingDatabases(true)
    setDatabasesError(null)
    setError(null)
    try {
      const found = await listConnectorDatabases({ projectId, connectorId: target })
      setDatabases(found)
    } catch (err) {
      setDatabasesError(err instanceof Error ? err.message : 'Could not list databases')
    } finally {
      setListingDatabases(false)
    }
  }

  const discover = async (id?: string) => {
    const target = id ?? connectorId
    if (!target || !projectId) {
      return
    }
    setDiscovering(true)
    setDiscoveryError(null)
    setError(null)
    try {
      const found = await discoverConnectorStreams({ projectId, connectorId: target })
      setStreams(found)
    } catch (err) {
      setDiscoveryError(err instanceof Error ? err.message : 'Discovery failed')
    } finally {
      setDiscovering(false)
    }
  }

  const goNext = async () => {
    setError(null)
    if (step === 'connection') {
      if (!canContinue) {
        setError('Please fill in the required connection fields.')
        return
      }
      let id: string
      try {
        id = await ensureConnector()
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Could not create connector')
        return
      }
      // Spec connectors have no databases step: discover streams directly.
      if (isSpec) {
        setStep('scope')
        void discover(id)
        return
      }
      // Auto-list databases on entering the Database step.
      setStep('database')
      void listDatabases(id)
      return
    }
    if (step === 'database') {
      if (!database) {
        setError('Pick a database to sync.')
        return
      }
      // Persist the chosen database, then auto-discover streams.
      try {
        const config = buildConfig()
        await updateConnector({ id: connectorId!, config })
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Could not save database')
        return
      }
      setStep('scope')
      void discover(connectorId!)
      return
    }
    if (stepIndex < steps.length - 1) {
      setStep(steps[stepIndex + 1].id)
    }
  }

  const goBack = () => {
    if (stepIndex > 0) {
      setStep(steps[stepIndex - 1].id)
    }
  }

  const toggleEntity = (key: string) => {
    setEntities((prev) => {
      const next = new Set(prev)
      if (next.has(key)) {
        next.delete(key)
      } else {
        next.add(key)
      }
      return next
    })
  }

  const allSelected = streams.length > 0 && streams.every((s) => entities.has(entityKey(s)))
  const toggleAll = () => {
    setEntities(allSelected ? new Set() : new Set(streams.map((s) => entityKey(s))))
  }

  const submit = async () => {
    if (!connectorId) {
      setError('Connector not created yet')
      return
    }
    setError(null)
    try {
      const config = buildConfig({ [scopeKey]: selectedEntities })
      await updateConnector({
        id: connectorId,
        config,
        transform: {
          naming: 'preserve',
          mergeStrategy: 'append',
          fields: { ignore: ignorePatterns }
        }
      })
      onClose()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Could not save connector')
    }
  }

  return (
    <div className="border-border mt-5 rounded-lg border bg-fill2">
      <div className="flex items-start justify-between border-b px-5 py-4">
        <div>
          <h3 className="flex items-center gap-2 text-base font-semibold">
            <Cable size={18} />
            Connect{' '}
            {isSpec ?
              sourceType
            : sourceType === 'postgres' ?
              'PostgreSQL'
            : sourceType === 'mysql' ?
              'MySQL'
            : 'MongoDB'}
          </h3>
          <p className="text-sm text-content2">
            {isSpec ?
              'Set up a paused connector — streams are discovered from the provider.'
            : 'Set up a paused connector — databases and streams are discovered automatically.'}
          </p>
        </div>
        <Button onClick={onClose} size="xsmall" variant="ghost">
          Close
        </Button>
      </div>

      <div className="grid grid-cols-1 gap-5 p-5 lg:grid-cols-[220px_1fr]">
        <ol className="space-y-2">
          {visibleSteps.map(({ id, label, icon: Icon }, index) => (
            <li
              className={cn(
                'flex items-center gap-2 rounded-md px-3 py-2 text-sm',
                step === id ? 'bg-secondary text-content' : 'text-content2'
              )}
              key={id}
            >
              <Icon size={15} />
              <span>
                {index + 1}. {label}
              </span>
            </li>
          ))}
        </ol>

        <div className="min-h-[260px]">
          {step === 'connection' && (
            <div className="grid gap-4">
              <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                <TextField
                  label={
                    <span className="flex items-center gap-1.5">
                      Name <RequiredHint />
                    </span>
                  }
                  size="small"
                  value={name}
                  onChange={(event) => setName(event.target.value)}
                />

                {isSpec && (
                  <div className="flex flex-col justify-end">
                    <p className="text-2xs text-content2/70">
                      API-base connector — OAuth/API token is stored write-only.
                    </p>
                  </div>
                )}

                {sourceType === 'mongodb' && (
                  <div className="flex flex-col justify-end">
                    <label className="flex cursor-pointer items-center gap-2 text-sm">
                      <input
                        type="checkbox"
                        checked={useUri}
                        onChange={(event) => setUseUri(event.target.checked)}
                        className="size-4 accent-secondary"
                      />
                      <span>Use connection string</span>
                    </label>
                    {useUri && (
                      <p className="text-2xs text-content2/70">
                        e.g. mongodb://user:pass@host:27017/?directConnection=true
                      </p>
                    )}
                  </div>
                )}
              </div>

              {isSpec && (
                <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                  <TextField
                    label={
                      <span className="flex items-center gap-1.5">
                        <Server size={14} />
                        Base URL <RequiredHint />
                      </span>
                    }
                    size="small"
                    placeholder="https://api.hubapi.com"
                    value={baseUrl}
                    onChange={(event) => setBaseUrl(event.target.value)}
                  />
                  <TextField
                    label={
                      <span className="flex items-center gap-1.5">
                        <KeyRound size={14} />
                        Access token <OptionalHint />
                      </span>
                    }
                    size="small"
                    type="password"
                    placeholder="OAuth or API token"
                    value={accessToken}
                    onChange={(event) => setAccessToken(event.target.value)}
                  />
                </div>
              )}

              {useUri && sourceType === 'mongodb' ?
                <div className="grid grid-cols-1 gap-3">
                  <TextField
                    label={
                      <span className="flex items-center gap-1.5">
                        Connection string <RequiredHint />
                      </span>
                    }
                    size="small"
                    placeholder="mongodb://user:pass@host:27017/?directConnection=true"
                    value={uri}
                    onChange={(event) => setUri(event.target.value)}
                  />
                </div>
              : <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                  <TextField
                    label={
                      <span className="flex items-center gap-1.5">
                        Host <RequiredHint />
                      </span>
                    }
                    size="small"
                    placeholder="db.example.com"
                    value={host}
                    onChange={(event) => setHost(event.target.value)}
                  />
                  <TextField
                    label={
                      <span className="flex items-center gap-1.5">
                        Port <OptionalHint />
                      </span>
                    }
                    size="small"
                    value={port}
                    onChange={(event) => setPort(event.target.value)}
                  />
                  <TextField
                    label={
                      <span className="flex items-center gap-1.5">
                        <UserRound size={14} />
                        User <OptionalHint />
                      </span>
                    }
                    size="small"
                    value={user}
                    onChange={(event) => setUser(event.target.value)}
                  />
                  <TextField
                    label={
                      <span className="flex items-center gap-1.5">
                        <KeyRound size={14} />
                        Password <OptionalHint />
                      </span>
                    }
                    size="small"
                    type="password"
                    value={password}
                    onChange={(event) => setPassword(event.target.value)}
                  />
                </div>
              }
            </div>
          )}

          {step === 'database' && (
            <div className="grid gap-3">
              <div className="flex items-center justify-between">
                <p className="text-sm text-content2">
                  Databases discovered on <span className="font-medium">{host || uri}</span>. Pick one to
                  sync.
                </p>
                <Button
                  onClick={() => listDatabases()}
                  disabled={listingDatabases || !connectorId}
                  size="xsmall"
                  variant="outline"
                >
                  <RefreshCw className={cn('mr-1.5 size-3.5', listingDatabases && 'animate-spin')} />
                  {listingDatabases ? 'Listing…' : 'Refresh'}
                </Button>
              </div>

              {listingDatabases && databases.length === 0 ?
                <div className="flex items-center justify-center gap-2 rounded-md border border-dashed p-8 text-sm text-content2">
                  <RefreshCw className="size-4 animate-spin" />
                  Discovering databases…
                </div>
              : databasesError ?
                <div className="grid gap-2 rounded-md border border-dashed p-6 text-center">
                  <p className="text-sm text-danger">{databasesError}</p>
                  <p className="text-xs text-content2">Check the connection details, then retry.</p>
                  <div className="mt-1">
                    <Button onClick={() => listDatabases()} size="xsmall" variant="outline">
                      <RefreshCw className="mr-1.5 size-3.5" />
                      Retry
                    </Button>
                  </div>
                </div>
              : databases.length === 0 ?
                <p className="rounded-md border border-dashed p-6 text-center text-sm text-content2">
                  No databases found on this server.
                </p>
              : <div className="max-h-[240px] overflow-auto rounded-md border">
                  {databases.map((db) => (
                    <label
                      className={cn(
                        'flex cursor-pointer items-center gap-3 border-b px-3 py-2 text-sm last:border-0',
                        database === db ? 'bg-secondary/60' : 'hover:bg-secondary/40'
                      )}
                      key={db}
                    >
                      <input
                        type="radio"
                        name="database"
                        checked={database === db}
                        onChange={() => setDatabase(db)}
                        className="size-4 accent-secondary"
                      />
                      <Database size={15} className="text-content2" />
                      <span className="font-medium">{db}</span>
                    </label>
                  ))}
                </div>
              }
            </div>
          )}

          {step === 'scope' && (
            <div className="grid gap-3">
              <div className="flex items-center justify-between">
                <p className="text-sm text-content2">
                  {isSpec ?
                    'Streams'
                  : sourceType === 'mongodb' ?
                    'Collections'
                  : 'Tables'}{' '}
                  in <span className="font-medium">{database}</span>, discovered automatically.
                </p>
                <Button
                  onClick={() => discover()}
                  disabled={discovering || !connectorId}
                  size="xsmall"
                  variant="outline"
                >
                  <RefreshCw className={cn('mr-1.5 size-3.5', discovering && 'animate-spin')} />
                  {discovering ? 'Discovering…' : 'Refresh'}
                </Button>
              </div>

              {discovering && streams.length === 0 ?
                <div className="flex items-center justify-center gap-2 rounded-md border border-dashed p-8 text-sm text-content2">
                  <RefreshCw className="size-4 animate-spin" />
                  Discovering streams…
                </div>
              : discoveryError ?
                <div className="grid gap-2 rounded-md border border-dashed p-6 text-center">
                  <p className="text-sm text-danger">{discoveryError}</p>
                  <p className="text-xs text-content2">Check the connection details, then retry.</p>
                  <div className="mt-1">
                    <Button onClick={() => discover()} size="xsmall" variant="outline">
                      <RefreshCw className="mr-1.5 size-3.5" />
                      Retry discovery
                    </Button>
                  </div>
                </div>
              : streams.length === 0 ?
                <p className="rounded-md border border-dashed p-6 text-center text-sm text-content2">
                  No streams found in “{database}”. Check that the database has{' '}
                  {isSpec ?
                    'streams'
                  : sourceType === 'mongodb' ?
                    'collections'
                  : 'tables'}
                  .
                </p>
              : <div className="max-h-[280px] overflow-auto rounded-md border">
                  <label className="flex cursor-pointer items-center gap-3 border-b bg-fill3/50 px-3 py-2 text-sm">
                    <input
                      type="checkbox"
                      checked={allSelected}
                      onChange={toggleAll}
                      className="size-4 accent-secondary"
                    />
                    <span className="font-medium">{allSelected ? 'Deselect all' : 'Select all'}</span>
                    <span className="ml-auto text-xs text-content2">
                      {streams.length}{' '}
                      {isSpec ?
                        'streams'
                      : sourceType === 'mongodb' ?
                        'collections'
                      : 'tables'}
                    </span>
                  </label>
                  {streams.map((stream) => {
                    const key = entityKey(stream)
                    const checked = entities.has(key)
                    return (
                      <label
                        className={cn(
                          'flex cursor-pointer items-center gap-3 border-b px-3 py-2 text-sm last:border-0',
                          checked ? 'bg-secondary/60' : 'hover:bg-secondary/40'
                        )}
                        key={key}
                      >
                        <input
                          type="checkbox"
                          checked={checked}
                          onChange={() => toggleEntity(key)}
                          className="size-4 accent-secondary"
                        />
                        <span className="font-mono text-xs text-content2">{stream.namespace}</span>
                        <span className="font-medium">{stream.name}</span>
                        {typeof stream.estimatedRecords === 'number' && (
                          <span className="ml-auto rounded bg-fill3 px-1.5 py-0.5 text-[11px] text-content2 tabular-nums">
                            {stream.estimatedRecords.toLocaleString()}{' '}
                            {stream.estimatedRecords === 1 ? 'record' : 'records'}
                          </span>
                        )}
                        {stream.targetLabel && (
                          <span className="rounded bg-fill3 px-1.5 py-0.5 text-[11px] text-content2">
                            {stream.targetLabel}
                          </span>
                        )}
                      </label>
                    )
                  })}
                </div>
              }

              <p className="text-sm text-content2">
                {selectedEntities.length ?
                  `${selectedEntities.length} selected`
                : 'None selected — the connector will sync nothing until you pick streams.'}
              </p>
            </div>
          )}

          {step === 'rules' && (
            <label className="flex flex-col gap-1 text-sm">
              <span className="font-medium">
                Ignore patterns <OptionalHint />
              </span>
              <span className="text-xs text-content2">
                Fields matching these patterns are never written to RushDB. One per line.
              </span>
              <textarea
                className="min-h-[160px] resize-none scrollbar-thin rounded-md border bg-secondary p-3 font-mono text-sm outline-hidden focus-visible:ring"
                placeholder={'email\npayment.**\n*.token'}
                value={ignore}
                onChange={(event) => setIgnore(event.target.value)}
              />
            </label>
          )}

          {step === 'review' && (
            <div className="grid gap-3 text-sm">
              <div className="rounded-md border p-3">
                <p className="font-semibold">{name}</p>
                <p className="text-content2">
                  {isSpec ?
                    `${sourceType} · ${baseUrl || 'default provider URL'}`
                  : `${sourceType} · ${useUri ? uri : `${host}:${port}`} · ${database}`}
                </p>
              </div>
              <div className="rounded-md border p-3">
                <p className="font-semibold">Scope</p>
                <p className="text-content2">
                  {selectedEntities.length ? selectedEntities.join(', ') : 'None selected'}
                </p>
              </div>
              <div className="rounded-md border p-3">
                <p className="font-semibold">Ignored fields</p>
                <p className="text-content2">{ignorePatterns.length ? ignorePatterns.join(', ') : 'None'}</p>
              </div>
              <p className="flex items-center gap-2 text-content2">
                <LockKeyhole size={14} />
                Secrets are write-only and will not be shown after saving.
              </p>
            </div>
          )}

          {error && <p className="mt-3 text-sm text-danger">{error}</p>}
        </div>
      </div>

      <div className="flex justify-between border-t px-5 py-4">
        <Button disabled={stepIndex === 0 || isPending} onClick={goBack} size="small" variant="outline">
          Back
        </Button>
        {step === 'review' ?
          <Button disabled={isPending} loading={isPending} onClick={submit} size="small" variant="primary">
            Save connector
          </Button>
        : <Button
            disabled={!canContinue || isPending}
            loading={isPending}
            onClick={goNext}
            size="small"
            variant="primary"
          >
            {step === 'database' ? 'Next' : 'Continue'}
          </Button>
        }
      </div>
    </div>
  )
}
