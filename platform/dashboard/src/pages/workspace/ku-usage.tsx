import { useStore } from '@nanostores/react'
import { Activity, BookOpen, TrendingUp, Zap } from 'lucide-react'

import { $currentWorkspace } from '~/features/workspaces/stores/current-workspace'
import { $currentWorkspacePlan } from '~/features/billing/stores/plans'
import { KU_PLAN_LABELS } from '~/features/billing/constants'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'

function formatKu(ku: number): string {
  if (ku >= 1_000_000_000) return `${(ku / 1_000_000_000).toFixed(1)}B`
  if (ku >= 1_000_000) return `${(ku / 1_000_000).toFixed(1)}M`
  if (ku >= 1_000) return `${(ku / 1_000).toFixed(0)}K`
  return ku.toLocaleString()
}

export default function KuUsagePage() {
  const workspace = useStore($currentWorkspace)
  const planState = useStore($currentWorkspacePlan)

  if (workspace.loading || planState.loading) {
    return (
      <div className="flex h-64 items-center justify-center">
        <div className="text-content3 text-sm">Loading usage data…</div>
      </div>
    )
  }

  const kuConsumed: number = (workspace.data as any)?.kuConsumed ?? 0
  const planId = workspace.data?.planId ?? 'free'
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
        <div className="bg-fill2 rounded-xl border p-4">
          <div className="mb-2 flex items-center gap-2">
            <Zap className="text-accent h-4 w-4" />
            <span className="text-content3 text-xs font-medium uppercase tracking-wide">KU Consumed</span>
          </div>
          <p className="text-content typography-2xl font-bold">{formatKu(kuConsumed)}</p>
          <p className="text-content3 mt-1 text-xs">this billing period</p>
        </div>

        <div className="bg-fill2 rounded-xl border p-4">
          <div className="mb-2 flex items-center gap-2">
            <Activity className="text-accent h-4 w-4" />
            <span className="text-content3 text-xs font-medium uppercase tracking-wide">KU Remaining</span>
          </div>
          <p className="text-content typography-2xl font-bold">
            {remaining !== null ? formatKu(remaining) : '∞'}
          </p>
          <p className="text-content3 mt-1 text-xs">until end of period</p>
        </div>

        <div className="bg-fill2 rounded-xl border p-4">
          <div className="mb-2 flex items-center gap-2">
            <BookOpen className="text-accent h-4 w-4" />
            <span className="text-content3 text-xs font-medium uppercase tracking-wide">Plan</span>
          </div>
          <p className="text-content typography-2xl font-bold capitalize">{planId}</p>
          <p className="text-content3 mt-1 text-xs">{planLabel}</p>
        </div>

        <div className="bg-fill2 rounded-xl border p-4">
          <div className="mb-2 flex items-center gap-2">
            <TrendingUp className="text-accent h-4 w-4" />
            <span className="text-content3 text-xs font-medium uppercase tracking-wide">Usage</span>
          </div>
          <p className="text-content typography-2xl font-bold">
            {kuLimit !== null ? `${usagePct.toFixed(1)}%` : 'N/A'}
          </p>
          <p className="text-content3 mt-1 text-xs">of monthly allowance</p>
        </div>
      </div>

      {/* Progress bar */}
      {kuLimit !== null && (
        <div className="bg-fill2 rounded-xl border p-6">
          <div className="mb-3 flex items-center justify-between">
            <h3 className="text-content font-semibold">Monthly Allowance</h3>
            <span className="text-content3 text-sm">
              {formatKu(kuConsumed)} / {formatKu(kuLimit)} KU
            </span>
          </div>
          <div className="bg-fill h-3 overflow-hidden rounded-full">
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
      <div className="bg-fill2 rounded-xl border p-6">
        <h3 className="text-content mb-4 font-semibold">What Generates KU?</h3>
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
              <div className="bg-accent mt-1.5 h-2 w-2 shrink-0 rounded-full" />
              <div>
                <p className="text-content text-sm font-medium">{label}</p>
                <p className="text-content3 text-xs">{desc}</p>
              </div>
            </div>
          ))}
        </div>
        <p className="text-content3 mt-4 text-xs">
          Internal KU weights per operation type are not exposed. You interact only with your total
          consumption.
        </p>
      </div>
    </div>
  )
}
