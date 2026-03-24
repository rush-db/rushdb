import { useEffect, useState } from 'react'
import { HelpCircle, X } from 'lucide-react'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { useWorkspaceProjectsQuery } from '~/features/workspaces/hooks/useWorkspaceQueries'
import { api } from '~/lib/api'
import { Skeleton } from '~/elements/Skeleton'
import { Message } from '~/elements/Message'
import { Button } from '~/elements/Button'
import { Divider } from '~/elements/Divider'
import { ButtonGroup } from '~/elements/ButtonGroup'
import { Tooltip } from '~/elements/Tooltip'
import { cn } from '~/lib/utils'

interface KuEvent {
  id: string
  workspaceId: string
  projectId: string
  operation: string
  kuConsumed: number
  metadata: Record<string, unknown> | null
  timestamp: string
}

const OPERATION_LABELS: Record<string, string> = {
  entity_created: 'Entity Created',
  embedding_generated: 'Embedding Generated',
  relationship_created: 'Relationship Created',
  storage_footprint: 'Daily Storage Footprint',
  compute_operation: 'Compute Operation',
  knowledge_deleted: 'Knowledge Deleted'
}

// Format metadata for display in the events list
function formatMetadata(operation: string, metadata: Record<string, unknown> | null): string {
  if (!metadata || Object.keys(metadata).length === 0) {
    return ''
  }

  // Storage footprint - show count and project breakdown
  if (operation === 'storage_footprint') {
    const count = metadata.count
    const projectBreakdown = metadata.projectBreakdown as Record<string, number> | undefined

    const parts: string[] = []
    if (typeof count === 'number') {
      parts.push(`${count.toLocaleString()} records`)
    }
    if (projectBreakdown) {
      const projectCount = Object.keys(projectBreakdown).length
      parts.push(`${projectCount} project${projectCount !== 1 ? 's' : ''}`)
    }
    return parts.join(' • ')
  }

  // Entity created - show property count and upsert status
  if (operation === 'entity_created') {
    const parts: string[] = []
    if (typeof metadata.propertyCount === 'number') {
      parts.push(`${metadata.propertyCount} properties`)
    }
    if (metadata.upsert === true) {
      parts.push('upsert')
    }
    return parts.join(' • ')
  }

  // Relationship created - show count
  if (operation === 'relationship_created' && typeof metadata.count === 'number') {
    return `${metadata.count.toLocaleString()} relationships`
  }

  // Query heavy operation - show type
  if (operation === 'compute_operation' && metadata.type) {
    return `type: ${metadata.type}`
  }

  // Default: show simple key-value pairs (excluding count and complex objects)
  return Object.entries(metadata)
    .filter(([key, value]) => {
      // Skip internal fields
      if (key === 'count' || key === 'source') return false
      // Skip complex objects
      if (value && typeof value === 'object') return false
      return true
    })
    .map(([key, value]) => `${key}: ${value}`)
    .join(' • ')
}

// ---------------------------------------------------------------------------
// KU formatting helper
// ---------------------------------------------------------------------------

function formatKu(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(n % 1_000_000 === 0 ? 0 : 1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(n % 1_000 === 0 ? 0 : 1)}K`
  return n.toLocaleString()
}

// Returns YYYY-MM-DD in the browser's local timezone — used for chart bucketing
// so that e.g. 1:06 AM Feb 25 local (= Feb 24 UTC) slots into Feb 25, not Feb 24.
function toLocalDateKey(date: Date): string {
  const y = date.getFullYear()
  const m = String(date.getMonth() + 1).padStart(2, '0')
  const d = String(date.getDate()).padStart(2, '0')
  return `${y}-${m}-${d}`
}

// ---------------------------------------------------------------------------
// KU Usage Bar
// ---------------------------------------------------------------------------

type UsageData = {
  plan: string
  kuConsumed: number
  kuLimit: number | null
  kuIncluded: number | null
  remaining: number | null
  billingModel: 'fixed' | 'overage' | 'usage'
  billingPeriodStart: string
}

function KuUsageBar({ usage }: { usage: UsageData }) {
  const { plan, kuConsumed, kuIncluded, remaining, billingModel, billingPeriodStart } = usage

  const periodStart = new Date(billingPeriodStart).toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric'
  })
  const periodEnd = (() => {
    const d = new Date(billingPeriodStart)
    d.setMonth(d.getMonth() + 1)
    return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
  })()

  // Scale / enterprise — usage-based or truly unlimited: just show a stat
  if (billingModel === 'usage' || (kuIncluded === null && billingModel !== 'fixed')) {
    return (
      <div className="bg-card rounded-md border p-5">
        <div className="mb-1 flex items-center justify-between">
          <span className="text-content text-sm font-medium capitalize">{plan} — current period</span>
          <span className="text-content3 text-xs">
            {periodStart} – {periodEnd}
          </span>
        </div>
        <div className="text-accent font-mono text-2xl font-semibold">{formatKu(kuConsumed)} KU</div>
        <div className="text-content3 mt-1 text-xs">consumed this billing period · usage-based pricing</div>
      </div>
    )
  }

  // Free / Pro — fixed cap or overage model with an included allowance
  const cap = kuIncluded!
  const pct = Math.min((kuConsumed / cap) * 100, 100)
  const isOver = kuConsumed > cap
  const overageKu = isOver ? kuConsumed - cap : 0

  const barColor =
    isOver ? 'bg-danger'
    : pct >= 90 ? 'bg-warning'
    : pct >= 70 ? 'bg-warning/70'
    : 'bg-accent'

  return (
    <div className="bg-card rounded-md border p-5">
      {/* Header row */}
      <div className="mb-3 flex items-center justify-between">
        <span className="text-content text-sm font-medium capitalize">
          {plan} plan · {periodStart} – {periodEnd}
        </span>
        <span
          className={cn(
            'font-mono text-sm font-semibold',
            isOver ? 'text-danger'
            : pct >= 90 ? 'text-warning'
            : 'text-content'
          )}
        >
          {formatKu(kuConsumed)} / {formatKu(cap)} KU
        </span>
      </div>

      {/* Progress bar */}
      <div className="bg-secondary h-2.5 w-full overflow-hidden rounded-full">
        <div className={cn('h-full rounded-full transition-all', barColor)} style={{ width: `${pct}%` }} />
      </div>

      {/* Footer row */}
      <div className="mt-2 flex items-center justify-between text-xs">
        {isOver ?
          <span className="text-danger font-medium">
            +{formatKu(overageKu)} KU overage{plan === 'pro' ? ' · billed at $3/M' : ''}
          </span>
        : remaining !== null ?
          <span className="text-content3">{formatKu(remaining)} KU remaining</span>
        : <span className="text-content3">Usage tracked</span>}
        <span className="text-content3">{pct.toFixed(1)}%</span>
      </div>
    </div>
  )
}

function KuUsageHeader({ size = 'large' }: { size?: 'small' | 'large' }) {
  return (
    <div className={cn('flex flex-col gap-0.5', size === 'large' ? 'my-4' : undefined)}>
      <div className="flex items-center gap-2">
        <h2 className={cn('text-content font-semibold', size === 'large' ? 'text-2xl' : 'text-xl')}>
          KU Usage History
        </h2>
        <Tooltip
          side="right"
          trigger={
            <button className="text-content3 hover:text-content transition-colors">
              <HelpCircle className="h-4 w-4" />
            </button>
          }
        >
          <div className="flex max-w-xs flex-col gap-2 p-1">
            <p className="font-medium">Knowledge Units (KU)</p>
            <p className="text-xs opacity-80">
              KU measure the structured knowledge RushDB creates — records ingested, relationships formed,
              embeddings stored, and complex queries (aggregations or traversals deeper than 2 levels) on
              shared instances. Standard reads never consume KU.
            </p>
            <div className="flex flex-col gap-1 pt-1">
              <a
                href="https://docs.rushdb.com/concepts/knowledge-units"
                target="_blank"
                rel="noreferrer"
                className="text-accent text-xs hover:underline"
              >
                → What generates KU?
              </a>
              <a
                href="https://docs.rushdb.com/concepts/billing-model"
                target="_blank"
                rel="noreferrer"
                className="text-accent text-xs hover:underline"
              >
                → Billing model
              </a>
            </div>
          </div>
        </Tooltip>
      </div>
      <p className="text-content3 text-sm">
        Knowledge Units consumed by ingestion, relationships, embeddings, and complex queries on shared
        instances.
      </p>
    </div>
  )
}

export function KuUsageHistory() {
  const { data: settings } = usePlatformSettings()
  const { data: projectsList = [] } = useWorkspaceProjectsQuery()

  // ---------- filters ----------
  const [filterProjectId, setFilterProjectId] = useState<string | null>(null)
  const [filterOperation, setFilterOperation] = useState<string | null>(null)

  // ---------- paginated events list ----------
  const [events, setEvents] = useState<KuEvent[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [hasMore, setHasMore] = useState(false)
  const [nextCursor, setNextCursor] = useState<string | null>(null)
  // ---------- chart data (separate fetch, full time-range) ----------
  const [chartEvents, setChartEvents] = useState<KuEvent[]>([])
  const [chartLoading, setChartLoading] = useState(true)

  // preset buttons: 1 = Today, 7, 30
  const [timeRange, setTimeRange] = useState<1 | 7 | 30>(7)
  // custom date-range (YYYY-MM-DD strings, both must be set to be active)
  const [customFrom, setCustomFrom] = useState('')
  const [customTo, setCustomTo] = useState('')

  const [usage, setUsage] = useState<UsageData | null>(null)

  const isCustomRange = customFrom !== '' && customTo !== ''

  // Derive the `since` / `before` ISO strings for a chart fetch based on
  // whichever range mode is currently active.
  const buildChartRange = (
    tr: 1 | 7 | 30 = timeRange,
    cfrom: string = customFrom,
    cto: string = customTo
  ): { since: string; before?: string } => {
    if (cfrom !== '' && cto !== '') {
      return {
        since: new Date(cfrom + 'T00:00:00').toISOString(),
        before: new Date(cto + 'T23:59:59.999').toISOString()
      }
    }
    const d = new Date()
    if (tr === 1) {
      d.setHours(0, 0, 0, 0) // midnight local = start of today
    } else {
      d.setDate(d.getDate() - tr)
    }
    return { since: d.toISOString() }
  }

  // Build the ordered list of YYYY-MM-DD keys that the chart should display.
  const getActiveDateKeys = (
    tr: 1 | 7 | 30 = timeRange,
    cfrom: string = customFrom,
    cto: string = customTo
  ): string[] => {
    if (cfrom !== '' && cto !== '') {
      const keys: string[] = []
      const end = new Date(cto + 'T00:00:00')
      for (const d = new Date(cfrom + 'T00:00:00'); d <= end; d.setDate(d.getDate() + 1)) {
        keys.push(toLocalDateKey(new Date(d)))
      }
      return keys
    }
    if (tr === 1) return [toLocalDateKey(new Date())]
    const keys: string[] = []
    const today = new Date()
    for (let i = tr - 1; i >= 0; i--) {
      const d = new Date(today)
      d.setDate(d.getDate() - i)
      keys.push(toLocalDateKey(d))
    }
    return keys
  }

  // Don't show in self-hosted mode
  if (settings?.selfHosted) {
    return null
  }

  const loadUsage = async () => {
    try {
      const data = await api.billing.getUsage()
      setUsage(data)
    } catch {
      // Non-critical — the progress bar is best-effort
    }
  }

  const loadEvents = async (
    cursor?: string,
    projectId: string | null = filterProjectId,
    operation: string | null = filterOperation
  ) => {
    try {
      setLoading(true)
      setError(null)

      const response = await api.billing.getKuHistory({
        limit: 50,
        before: cursor,
        projectId,
        operation
      })

      if (cursor) {
        // Append to existing events (pagination)
        setEvents((prev) => [...prev, ...response.events])
      } else {
        // Initial load or filter reset
        setEvents(response.events)
      }

      setHasMore(response.hasMore)
      setNextCursor(response.nextCursor)
    } catch (err: any) {
      setError(err.message || 'Failed to load KU usage history')
    } finally {
      setLoading(false)
    }
  }

  // Fetch ALL events in the selected time window for the chart.
  // Uses `since` (+ optional `before`) so the backend returns up to 1000 events
  // bounded by the date range rather than a hard cursor-paged 50-item slice.
  const loadChartData = async (
    range: { since: string; before?: string },
    projectId: string | null = filterProjectId,
    operation: string | null = filterOperation
  ) => {
    try {
      setChartLoading(true)
      const response = await api.billing.getKuHistory({
        since: range.since,
        before: range.before,
        limit: 1000,
        projectId,
        operation
      })
      setChartEvents(response.events)
    } catch {
      // Non-critical — chart falls back to empty bars
    } finally {
      setChartLoading(false)
    }
  }

  useEffect(() => {
    loadUsage()
    loadEvents()
  }, [])

  // Re-fetch both list and chart when filters change; reset pagination cursor
  useEffect(() => {
    setNextCursor(null)
    loadEvents(undefined, filterProjectId, filterOperation)
    loadChartData(buildChartRange(), filterProjectId, filterOperation)
  }, [filterProjectId, filterOperation])

  useEffect(() => {
    loadChartData(buildChartRange(timeRange, customFrom, customTo))
  }, [timeRange, customFrom, customTo])

  // Calculate daily aggregates for bar chart — uses chartEvents (full range),
  // NOT the paginated events list.
  const getDailyAggregates = () => {
    const dailyMap = new Map<string, number>()
    chartEvents.forEach((event) => {
      const key = toLocalDateKey(new Date(event.timestamp))
      dailyMap.set(key, (dailyMap.get(key) || 0) + event.kuConsumed)
    })

    return getActiveDateKeys().map((key) => ({ date: key, ku: dailyMap.get(key) || 0 }))
  }

  const dailyData = getDailyAggregates()
  const maxKu = Math.max(...dailyData.map((d) => d.ku), 1)

  if (loading && events.length === 0) {
    return (
      <div className="flex flex-col gap-5">
        <KuUsageHeader size="small" />
        <Skeleton enabled className="h-24 w-full rounded-md" />
        <Skeleton enabled className="h-48 w-full rounded-md" />
        <Skeleton enabled className="h-64 w-full rounded-md" />
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex flex-col gap-5">
        <KuUsageHeader size="small" />
        {usage && <KuUsageBar usage={usage} />}
        <Message variant="danger" size="medium">
          {error}
        </Message>
      </div>
    )
  }

  // True first-load empty (no filters active, never had data)
  if (events.length === 0 && !loading && !filterOperation && !filterProjectId) {
    return (
      <div className="flex flex-col gap-5">
        <KuUsageHeader size="small" />
        {usage && <KuUsageBar usage={usage} />}
        <Message variant="info" size="medium">
          No KU usage recorded yet. Start using your workspace to see usage data here.
        </Message>
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-5">
      <KuUsageHeader />
      {usage && <KuUsageBar usage={usage} />}

      {/* Filter bar */}
      <div className="bg-card flex flex-wrap items-center gap-3 rounded-md border p-3">
        <span className="text-content3 text-sm font-medium">Filter</span>

        {/* Operation type */}
        <select
          value={filterOperation ?? ''}
          onChange={(e) => setFilterOperation(e.target.value || null)}
          className="bg-secondary text-content border-border rounded-md border px-2.5 py-1.5 text-sm focus:outline-none"
        >
          <option value="">All operation types</option>
          {Object.entries(OPERATION_LABELS).map(([key, label]) => (
            <option key={key} value={key}>
              {label}
            </option>
          ))}
        </select>

        {/* Project */}
        {projectsList.length > 0 && (
          <select
            value={filterProjectId ?? ''}
            onChange={(e) => setFilterProjectId(e.target.value || null)}
            className="bg-secondary text-content border-border rounded-md border px-2.5 py-1.5 text-sm focus:outline-none"
          >
            <option value="">All projects</option>
            {projectsList.map((project) => (
              <option key={project.id} value={project.id}>
                {project.name}
              </option>
            ))}
          </select>
        )}

        {/* Clear button */}
        {(filterOperation || filterProjectId) && (
          <button
            onClick={() => {
              setFilterOperation(null)
              setFilterProjectId(null)
            }}
            className="text-content3 hover:text-content flex items-center gap-1 text-xs transition-colors"
          >
            <X className="h-3 w-3" />
            Clear filters
          </button>
        )}
      </div>

      {/* Bar Chart */}
      {chartLoading ?
        <Skeleton enabled className="h-64 w-full rounded-md" />
      : dailyData.length > 0 && (
          <div className="bg-card flex flex-col gap-5 rounded-md border p-5">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <h3 className="text-content text-lg font-medium">Daily KU Consumption</h3>
              <div className="flex flex-wrap items-center gap-2">
                {/* Preset buttons — clicking one clears any custom range */}
                <ButtonGroup>
                  <Button
                    size="xsmall"
                    variant={!isCustomRange && timeRange === 1 ? 'secondary' : 'ghost'}
                    onClick={() => {
                      setCustomFrom('')
                      setCustomTo('')
                      setTimeRange(1)
                    }}
                  >
                    Today
                  </Button>
                  <Button
                    size="xsmall"
                    variant={!isCustomRange && timeRange === 7 ? 'secondary' : 'ghost'}
                    onClick={() => {
                      setCustomFrom('')
                      setCustomTo('')
                      setTimeRange(7)
                    }}
                  >
                    7 days
                  </Button>
                  <Button
                    size="xsmall"
                    variant={!isCustomRange && timeRange === 30 ? 'secondary' : 'ghost'}
                    onClick={() => {
                      setCustomFrom('')
                      setCustomTo('')
                      setTimeRange(30)
                    }}
                  >
                    30 days
                  </Button>
                </ButtonGroup>

                {/* Custom date-range — max 30 days span */}
                <div className="flex items-center gap-1.5">
                  <input
                    type="date"
                    value={customFrom}
                    max={customTo || toLocalDateKey(new Date())}
                    onChange={(e) => {
                      const val = e.target.value
                      setCustomFrom(val)
                      // enforce 30-day max: clamp `to` if needed
                      if (val && customTo) {
                        const maxTo = new Date(val + 'T00:00:00')
                        maxTo.setDate(maxTo.getDate() + 30)
                        if (new Date(customTo + 'T00:00:00') > maxTo) {
                          setCustomTo(toLocalDateKey(maxTo))
                        }
                      }
                    }}
                    className="bg-secondary text-content border-border rounded-md border px-2 py-1 text-xs focus:outline-none"
                  />
                  <span className="text-content3 text-xs">—</span>
                  <input
                    type="date"
                    value={customTo}
                    min={customFrom || undefined}
                    max={(() => {
                      const cap = new Date()
                      if (customFrom) {
                        const maxTo = new Date(customFrom + 'T00:00:00')
                        maxTo.setDate(maxTo.getDate() + 30)
                        return toLocalDateKey(maxTo < cap ? maxTo : cap)
                      }
                      return toLocalDateKey(cap)
                    })()}
                    onChange={(e) => setCustomTo(e.target.value)}
                    className="bg-secondary text-content border-border rounded-md border px-2 py-1 text-xs focus:outline-none"
                  />
                  {isCustomRange && (
                    <button
                      onClick={() => {
                        setCustomFrom('')
                        setCustomTo('')
                      }}
                      className="text-content3 hover:text-content transition-colors"
                      title="Clear custom range"
                    >
                      <X className="h-3 w-3" />
                    </button>
                  )}
                </div>
              </div>
            </div>
            <div className="flex h-64 items-end justify-between gap-2">
              {dailyData.map((day) => (
                <div key={day.date} className="flex flex-1 flex-col items-center gap-2">
                  <Tooltip
                    side="top"
                    trigger={
                      <div className="relative w-full">
                        <div
                          className="bg-accent hover:bg-accent-hover cursor-pointer rounded-t-md transition-all"
                          style={{
                            height: `${(day.ku / maxKu) * 200}px`,
                            minHeight: day.ku > 0 ? '4px' : '0px'
                          }}
                        />
                      </div>
                    }
                  >
                    <div className="flex flex-col gap-1">
                      <div className="font-medium">
                        {new Date(day.date + 'T00:00:00').toLocaleDateString('en-US', {
                          weekday: 'short',
                          month: 'short',
                          day: 'numeric'
                        })}
                      </div>
                      <div className="font-mono font-semibold">{day.ku.toFixed(2)} KU</div>
                    </div>
                  </Tooltip>
                  <div className="text-content3 text-xs">
                    {new Date(day.date + 'T00:00:00').toLocaleDateString('en-US', {
                      month: 'short',
                      day: 'numeric'
                    })}
                  </div>
                  <div className="text-content font-mono text-xs font-medium">{formatKu(day.ku)}</div>
                </div>
              ))}
            </div>
          </div>
        )
      }

      {/* Events List */}
      <div className="bg-card flex flex-col rounded-md border">
        <div className="flex items-center justify-between p-5">
          <h3 className="text-content text-lg font-medium">Recent Operations</h3>
        </div>
        <Divider />
        <div className="flex flex-col">
          {loading && events.length === 0 ?
            <div className="p-5">
              <Skeleton enabled className="h-10 w-full rounded-md" />
            </div>
          : events.length === 0 ?
            <div className="p-5">
              <Message variant="info" size="medium">
                No operations match the selected filters.
              </Message>
            </div>
          : events.map((event, index) => (
              <div key={event.id}>
                {index > 0 && <Divider />}
                <div className="hover:bg-secondary flex items-center justify-between p-5 transition-colors">
                  <div className="flex flex-1 flex-col gap-1">
                    <div className="text-content font-medium">
                      {OPERATION_LABELS[event.operation] || event.operation}
                    </div>
                    <div className="text-content3 text-sm">
                      {new Date(event.timestamp).toLocaleString()}
                      {event.metadata && Object.keys(event.metadata).length > 0 && (
                        <span className="text-content2 ml-2">
                          {formatMetadata(event.operation, event.metadata)}
                        </span>
                      )}
                    </div>
                  </div>
                  <div className="ml-4 text-right">
                    <div className="text-accent font-mono font-semibold">
                      {event.kuConsumed.toFixed(3)} KU
                    </div>
                  </div>
                </div>
              </div>
            ))
          }
        </div>

        {/* Load More Button */}
        {hasMore && (
          <>
            <Divider />
            <div className="p-5">
              <Button
                variant="secondary"
                size="medium"
                className="w-full justify-center"
                onClick={() => loadEvents(nextCursor || undefined)}
                disabled={loading}
                loading={loading}
              >
                Load More
              </Button>
            </div>
          </>
        )}
      </div>
    </div>
  )
}
