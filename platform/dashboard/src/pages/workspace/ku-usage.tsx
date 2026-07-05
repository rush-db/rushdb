import { Activity, BookOpen, TrendingUp, Zap } from 'lucide-react'

import { useCurrentWorkspaceQuery } from '~/features/workspaces/hooks/useWorkspaceQueries'
import { useCurrentWorkspacePlan } from '~/features/billing/hooks/useBillingHooks'
import { KU_PLAN_LABELS } from '~/features/billing/constants'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'

function formatKu(ku: number): string {
  if (ku >= 1_000_000_000) return `${(ku / 1_000_000_000).toFixed(1)}B`
  if (ku >= 1_000_000) return `${(ku / 1_000_000).toFixed(1)}M`
  if (ku >= 1_000) return `${(ku / 1_000).toFixed(0)}K`
  return ku.toLocaleString()
}

export default function KuUsagePage() {
  const { data: workspace, isPending: workspaceLoading } = useCurrentWorkspaceQuery()
  const { currentPlan, loading: planLoading } = useCurrentWorkspacePlan()

  if (workspaceLoading || planLoading) {
    return (
      <div className="flex h-64 items-center justify-center">
        <div className="text-sm text-content3">Loading usage data…</div>
      </div>
    )
  }

  const kuConsumed: number = (workspace as any)?.kuConsumed ?? 0
  const planId = workspace?.planId ?? 'free'
  const planLabel = KU_PLAN_LABELS[planId] ?? '—'

  // Derive kuLimit from plan label (free=100k, pro=10m, scale/enterprise=null)
  const kuLimit: number | null =
    planId === 'free' || planId === 'start' ? 100_000
    : planId === 'pro' ? 10_000_000
    : null

  const usagePct = kuLimit !== null ? Math.min((kuConsumed / kuLimit) * 100, 100) : 0
  const remaining = kuLimit !== null ? Math.max(kuLimit - kuConsumed, 0) : null

  const barColor =
    usagePct >= 90 ? '#ef4444'
    : usagePct >= 70 ? '#f59e0b'
    : 'hsl(72.96 82.69% 61.55%)'

  return (
    <div className="mx-auto max-w-4xl space-y-6 p-6">
      <PageTitle>Knowledge Unit Usage</PageTitle>

      {/* Summary cards */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <div className="rounded-xl border bg-fill2 p-4">
          <div className="mb-2 flex items-center gap-2">
            <Zap className="h-4 w-4 text-accent" />
            <span className="text-xs font-medium tracking-wide text-content3 uppercase">KU Consumed</span>
          </div>
          <p className="typography-2xl font-bold text-content">{formatKu(kuConsumed)}</p>
          <p className="mt-1 text-xs text-content3">this billing period</p>
        </div>

        <div className="rounded-xl border bg-fill2 p-4">
          <div className="mb-2 flex items-center gap-2">
            <Activity className="h-4 w-4 text-accent" />
            <span className="text-xs font-medium tracking-wide text-content3 uppercase">KU Remaining</span>
          </div>
          <p className="typography-2xl font-bold text-content">
            {remaining !== null ? formatKu(remaining) : '∞'}
          </p>
          <p className="mt-1 text-xs text-content3">until end of period</p>
        </div>

        <div className="rounded-xl border bg-fill2 p-4">
          <div className="mb-2 flex items-center gap-2">
            <BookOpen className="h-4 w-4 text-accent" />
            <span className="text-xs font-medium tracking-wide text-content3 uppercase">Plan</span>
          </div>
          <p className="typography-2xl font-bold text-content capitalize">{planId}</p>
          <p className="mt-1 text-xs text-content3">{planLabel}</p>
        </div>

        <div className="rounded-xl border bg-fill2 p-4">
          <div className="mb-2 flex items-center gap-2">
            <TrendingUp className="h-4 w-4 text-accent" />
            <span className="text-xs font-medium tracking-wide text-content3 uppercase">Usage</span>
          </div>
          <p className="typography-2xl font-bold text-content">
            {kuLimit !== null ? `${usagePct.toFixed(1)}%` : 'N/A'}
          </p>
          <p className="mt-1 text-xs text-content3">of monthly allowance</p>
        </div>
      </div>

      {/* Progress bar */}
      {kuLimit !== null && (
        <div className="rounded-xl border bg-fill2 p-6">
          <div className="mb-3 flex items-center justify-between">
            <h3 className="font-semibold text-content">Monthly Allowance</h3>
            <span className="text-sm text-content3">
              {formatKu(kuConsumed)} / {formatKu(kuLimit)} KU
            </span>
          </div>
          <div className="h-3 overflow-hidden rounded-full bg-fill">
            <div
              className="h-3 rounded-full transition-all duration-500"
              style={{ width: `${usagePct}%`, backgroundColor: barColor }}
            />
          </div>
          {usagePct >= 80 && (
            <p className="mt-3 text-sm" style={{ color: barColor }}>
              {usagePct >= 100 ?
                'Your KU allowance is exhausted. Upgrade to continue writing data.'
              : `You have used ${usagePct.toFixed(0)}% of your monthly KU allowance.`}
            </p>
          )}
        </div>
      )}

      {/* What generates KU */}
      <div className="rounded-xl border bg-fill2 p-6">
        <h3 className="mb-4 font-semibold text-content">What Generates KU?</h3>
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
          {[
            { label: 'Record ingestion', desc: 'Creating or upserting records via JSON/CSV import' },
            { label: 'Relationship creation', desc: 'Attaching records to build the knowledge graph' },
            { label: 'Vector embeddings', desc: 'Storing and indexing numeric vector arrays' },
            { label: 'Heavy queries', desc: 'Multi-hop traversals and raw query execution' },
            { label: 'Stored footprint', desc: 'Ongoing KU for knowledge maintained in your workspace' },
            { label: 'Deletions', desc: 'Stop ongoing footprint KU (creation KU is not reversed)' }
          ].map(({ label, desc }) => (
            <div key={label} className="flex gap-3">
              <div className="mt-1.5 h-2 w-2 shrink-0 rounded-full bg-accent" />
              <div>
                <p className="text-sm font-medium text-content">{label}</p>
                <p className="text-xs text-content3">{desc}</p>
              </div>
            </div>
          ))}
        </div>
        <p className="mt-4 text-xs text-content3">
          Internal KU weights per operation type are not exposed. You interact only with your total
          consumption.
        </p>
      </div>
    </div>
  )
}
