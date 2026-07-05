import { Ban, Check, ExternalLink, Info, List, Lock, Network, Trash2 } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { useStore } from '@nanostores/react'

import type { Project } from '~/features/projects/types'
import type { ExistingRelationshipSummary, RelationshipPattern } from '~/features/relationship-patterns/types'
import type { GraphOutput } from '~/features/projects/components/GraphCanvas'

import { Badge } from '~/elements/Badge'
import { Button } from '~/elements/Button'
import { Card } from '~/elements/Card'
import { Checkbox } from '~/elements/Checkbox'
import { ConfirmDialog } from '~/elements/ConfirmDialog'
import { Close, Dialog, DialogFooter, DialogTitle } from '~/elements/Dialog'
import { IconButton } from '~/elements/IconButton'
import { Label } from '~/elements/Label'
import { NothingFound } from '~/elements/NothingFound'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { Skeleton } from '~/elements/Skeleton'
import { Tab, Tabs, TabsList } from '~/elements/Tabs'
import { Tooltip } from '~/elements/Tooltip'
import {
  useApproveRelationshipPatternMutation,
  useApproveRelationshipPatternsMutation,
  useDeleteRelationshipPatternMutation,
  useIgnoreRelationshipPatternMutation,
  useRelationshipPatternsQuery
} from '~/features/relationship-patterns/hooks'
import { $currentProjectId } from '~/features/projects/stores/id'
import { $tourStep, setTourStep } from '~/features/tour/stores/tour'
import { getLabelColor } from '~/features/labels/utils'
import { GraphCanvas } from '~/features/projects/components/GraphCanvas'
import { changeSearchParam, openRoute } from '~/lib/router'

// Relationship type RushDB assigns to edges it creates automatically while importing
// nested data. These are part of the imported structure and aren't managed from here.
const RUSHDB_DEFAULT_RELATION_TYPE = '__RUSHDB__RELATION__DEFAULT__'

// A relationship shown in the top section. `pattern` is present only when the edge
// is backed by an approved suggestion — those stay user-deletable, while edges that
// emerged from imports (no pattern) are read-only.
type RelationshipRowData = {
  key: string
  sourceLabel: string
  targetLabel: string
  type: string
  pattern?: RelationshipPattern
}

function relationshipEdgeKey(sourceLabel: string, type: string, targetLabel: string) {
  return `${sourceLabel}|${type}|${targetLabel}`
}

// Merge import-derived existing relationships with approved patterns into a single
// deduped list. The backend already normalizes existing edges to source → type →
// target and approved patterns apply with the same orientation, so one key dedupes
// both. Approved patterns not yet reflected in the schema still appear here.
function buildRelationshipRows(
  relationships: ExistingRelationshipSummary[],
  approved: RelationshipPattern[]
): RelationshipRowData[] {
  const rows = new Map<string, RelationshipRowData>()

  relationships.forEach((item) => {
    item.relationships.forEach((relationship) => {
      const sourceLabel = relationship.direction === 'in' ? relationship.label : item.label
      const targetLabel = relationship.direction === 'in' ? item.label : relationship.label
      const key = relationshipEdgeKey(sourceLabel, relationship.type, targetLabel)
      if (!rows.has(key)) {
        rows.set(key, { key, sourceLabel, targetLabel, type: relationship.type })
      }
    })
  })

  approved.forEach((pattern) => {
    const key = relationshipEdgeKey(pattern.source.label, pattern.type, pattern.target.label)
    const existing = rows.get(key)
    if (existing) {
      existing.pattern = pattern
    } else {
      rows.set(key, {
        key,
        sourceLabel: pattern.source.label,
        targetLabel: pattern.target.label,
        type: pattern.type,
        pattern
      })
    }
  })

  return [...rows.values()].sort((a, b) => a.key.localeCompare(b.key))
}

// The label-level schema graph: each label rendered once, every deduped
// relationship type as a directed edge. Reuses the Records GraphCanvas renderer.
function buildSchemaGraph(rows: RelationshipRowData[], labelNames: string[]): GraphOutput {
  const nodeLabels = new Set<string>()
  rows.forEach((row) => {
    nodeLabels.add(row.sourceLabel)
    nodeLabels.add(row.targetLabel)
  })

  const nodes = [...nodeLabels].map((name) => ({
    id: `label:${name}`,
    kind: 'record' as const,
    label: name,
    color: getLabelColor(name, labelNames.indexOf(name))
  }))

  const links = rows.map((row) => ({
    id: `rel:${row.key}`,
    kind: 'record-relation' as const,
    source: `label:${row.sourceLabel}`,
    target: `label:${row.targetLabel}`,
    relationType: row.type
  }))

  return { nodes, links }
}

function schemaNodeHoverLabel(node: { label: string }) {
  const shellStyle =
    'background: rgba(0, 0, 0, 0.5); color: #fff; border: 1px solid rgba(255, 255, 255, 0.2); border-radius: 8px; padding: 8px 10px; line-height: 1.35; font-size: 12px; backdrop-filter: blur(2px); max-width: 320px;'
  return `<div style="${shellStyle}"><div><b>Label</b>: ${node.label}</div></div>`
}

const RELATIONSHIP_DOCS_URL = 'https://docs.rushdb.com/learn/relationships'
export const SUGGESTED_PATTERNS_DOCS_URL = 'https://docs.rushdb.com/learn/relationships/suggested-patterns'

// Collect every label referenced by existing relationships and patterns, sorted, for
// consistent label coloring across the relationships and suggested-relationships pages.
export function relationshipLabelNames(
  patterns: RelationshipPattern[],
  relationships: ExistingRelationshipSummary[]
): string[] {
  const labels = new Set<string>()

  relationships.forEach((item) => {
    labels.add(item.label)
    item.relationships.forEach((relationship) => labels.add(relationship.label))
  })
  patterns.forEach((pattern) => {
    labels.add(pattern.source.label)
    labels.add(pattern.target.label)
  })

  return [...labels].sort()
}

function RelationshipPath({
  sourceLabel,
  sourceKey,
  targetLabel,
  targetKey,
  type,
  labelNames
}: {
  labelNames: string[]
  sourceLabel: string
  sourceKey?: string
  targetLabel: string
  targetKey?: string
  type: string
}) {
  const sourceVariant = getLabelColor(sourceLabel, labelNames.indexOf(sourceLabel))
  const targetVariant = getLabelColor(targetLabel, labelNames.indexOf(targetLabel))

  return (
    <div className="flex flex-wrap items-center gap-2 font-mono text-sm">
      <LabelWithKey label={sourceLabel} propertyKey={sourceKey} variant={sourceVariant} />
      <span className="text-content3">→</span>
      <Badge className="text-sm!">{type}</Badge>
      <span className="text-content3">→</span>
      <LabelWithKey label={targetLabel} propertyKey={targetKey} variant={targetVariant} />
    </div>
  )
}

// Renders a colored label badge alongside its matched field as a separate gray
// monospace chip — mirroring the Suggested embedding indexes layout, so the field
// reads as a property of the label rather than part of the (colored) label itself.
function LabelWithKey({
  label,
  propertyKey,
  variant
}: {
  label: string
  propertyKey?: string
  variant: ReturnType<typeof getLabelColor>
}) {
  return (
    <span className="inline-flex items-center gap-1.5">
      <Label className="text-sm!" variant={variant}>
        {label}
      </Label>
      {propertyKey ?
        <>
          <span className="text-content3">:</span>
          <span className="w-fit rounded-sm bg-content3/10 px-1 font-mono text-xs text-content2">
            {propertyKey}
          </span>
        </>
      : null}
    </span>
  )
}

function PatternPath({ labelNames, pattern }: { labelNames: string[]; pattern: RelationshipPattern }) {
  return (
    <div className="flex flex-wrap items-center gap-3">
      <RelationshipPath
        labelNames={labelNames}
        sourceLabel={pattern.source.label}
        sourceKey={pattern.source.key}
        targetLabel={pattern.target.label}
        targetKey={pattern.target.key}
        type={pattern.type}
      />
      <Badge className="text-sm text-content2">
        {pattern.mode === 'retype_existing_relationship' ? 'Rename existing' : 'Match fields'}
      </Badge>
    </div>
  )
}

function DeletePatternDialog({
  pattern,
  onDelete,
  loading
}: {
  pattern: RelationshipPattern
  onDelete: (deleteExisting: boolean) => Promise<unknown>
  loading: boolean
}) {
  const [open, setOpen] = useState(false)
  const [deleteExisting, setDeleteExisting] = useState(true)

  return (
    <Dialog
      className="justify-center gap-4"
      onOpenChange={(nextOpen) => {
        setOpen(nextOpen)
        if (nextOpen) {
          setDeleteExisting(true)
        }
      }}
      open={open}
      trigger={
        <IconButton aria-label="Delete pattern" size="small" variant="dangerGhost">
          <Trash2 />
        </IconButton>
      }
    >
      <DialogTitle className="text-base font-bold">Delete relationship pattern</DialogTitle>
      <div className="flex flex-col gap-4">
        <p className="text-content2">Future writes will stop using this relationship pattern.</p>
        <div className="flex items-start gap-3 rounded-md border p-3">
          <Checkbox
            checked={deleteExisting}
            className="mt-0.5"
            id={`delete-existing-${pattern.id}`}
            onCheckedChange={(checked) => setDeleteExisting(checked === true)}
          />
          <label className="text-sm" htmlFor={`delete-existing-${pattern.id}`}>
            Also Delete created relationships by this pattern
          </label>
        </div>
      </div>
      <DialogFooter className="mt-3 flex-col sm:flex-row">
        <Button
          className="sm:order-2 sm:flex-1"
          loading={loading}
          onClick={() => onDelete(deleteExisting).then(() => setOpen(false))}
          variant="danger"
        >
          Delete
        </Button>
        <Close asChild disabled={loading}>
          <Button className="sm:flex-1" variant="secondary">
            Cancel
          </Button>
        </Close>
      </DialogFooter>
    </Dialog>
  )
}

export function isAgentRunJoinPattern(pattern: RelationshipPattern) {
  const labels = [pattern.source.label, pattern.target.label].map((label) => label.toUpperCase())
  const keys = [pattern.source.key, pattern.target.key].map((key) => key?.toLowerCase())

  return (
    pattern.mode === 'join_pattern' &&
    labels.some((label) => label.includes('AGENT')) &&
    labels.some((label) => label.includes('RUN')) &&
    keys.includes('agentid')
  )
}

export function sortSuggestedPatterns(patterns: RelationshipPattern[]) {
  return [...patterns].sort((a, b) => {
    const aScore = isAgentRunJoinPattern(a) ? 1 : 0
    const bScore = isAgentRunJoinPattern(b) ? 1 : 0
    if (bScore !== aScore) return bScore - aScore
    return b.confidence - a.confidence
  })
}

function PatternCard({
  labelNames,
  pattern,
  tourApproveTarget = false,
  selectable = false,
  selected = false,
  onToggleSelected
}: {
  labelNames: string[]
  pattern: RelationshipPattern
  tourApproveTarget?: boolean
  selectable?: boolean
  selected?: boolean
  onToggleSelected?: (id: string, checked: boolean) => void
}) {
  const approve = useApproveRelationshipPatternMutation()
  const ignore = useIgnoreRelationshipPatternMutation()
  const remove = useDeleteRelationshipPatternMutation()
  const projectId = useStore($currentProjectId)
  const tourStep = useStore($tourStep)

  return (
    <li className="bg-card rounded-md border px-4 py-3">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex min-w-0 items-start gap-3">
          {selectable && pattern.status === 'suggested' ?
            <Checkbox
              aria-label="Select relationship pattern"
              checked={selected}
              className="mt-0.5"
              onCheckedChange={(checked) => onToggleSelected?.(pattern.id, checked === true)}
            />
          : null}
          <div className="flex min-w-0 flex-col gap-1.5">
            <PatternPath labelNames={labelNames} pattern={pattern} />
            {pattern.rationale ?
              <p className="text-sm leading-snug text-content2">{pattern.rationale}</p>
            : null}
          </div>
        </div>
        <div className="flex shrink-0 flex-wrap items-center justify-end gap-3">
          {pattern.status === 'suggested' || pattern.status === 'ignored' || pattern.status === 'error' ?
            <div className="flex items-center gap-1 text-sm text-content2">
              <span>{Math.round(pattern.confidence * 100)}%</span>
              <Tooltip
                align="end"
                trigger={
                  <button
                    aria-label="Confidence score details"
                    className="inline-flex h-5 w-5 items-center justify-center text-content3 hover:text-content"
                    type="button"
                  >
                    <Info size={14} />
                  </button>
                }
              >
                <span className="font-semibold">Confidence Score</span>
                <span className="max-w-[320px] text-sm text-content2">
                  How likely this suggested relationship is correct, based on the analyzed data.
                </span>
                {pattern.lastError ?
                  <span className="max-w-[320px] text-sm text-danger">{pattern.lastError}</span>
                : null}
              </Tooltip>
            </div>
          : null}
          {pattern.status === 'suggested' ?
            <>
              <Button loading={ignore.isPending} onClick={() => ignore.mutate(pattern.id)} size="small">
                <Ban />
                Ignore
              </Button>
              <Button
                data-tour={tourApproveTarget ? 'project-relationships-approve' : undefined}
                loading={approve.isPending}
                onClick={() =>
                  approve.mutate(pattern.id, {
                    onSuccess: () => {
                      if (tourStep === 'projectRelationshipApprove' && projectId) {
                        openRoute('project', { id: projectId })
                        window.setTimeout(() => changeSearchParam('view', 'graph'), 0)
                        setTourStep('recordGraphView', true)
                      }
                    }
                  })
                }
                size="small"
                variant="primary"
              >
                <Check />
                Approve
              </Button>
            </>
          : null}
          {pattern.status === 'approved' ?
            <DeletePatternDialog
              loading={remove.isPending}
              onDelete={(deleteExisting) => remove.mutateAsync({ id: pattern.id, deleteExisting })}
              pattern={pattern}
            />
          : null}
          {pattern.status === 'ignored' || pattern.status === 'error' ?
            <ConfirmDialog
              handler={() => remove.mutateAsync({ id: pattern.id })}
              title="Delete ignored pattern"
              description="This removes the ignored state. If the same relationship is suggested again in a future analysis, it will appear as a suggestion."
              trigger={
                <Button size="small" variant="secondary">
                  <Trash2 />
                  Delete pattern
                </Button>
              }
            />
          : null}
        </div>
      </div>
    </li>
  )
}

function RelationshipRow({ labelNames, row }: { labelNames: string[]; row: RelationshipRowData }) {
  const remove = useDeleteRelationshipPatternMutation()
  const pattern = row.pattern
  const applying = !!pattern && !pattern.lastAppliedAt && !pattern.lastError

  // Approved patterns show their LLM rationale; relationships that came in with the
  // imported data structure aren't editable here and explain themselves instead.
  const detail =
    pattern ? pattern.rationale
    : row.type === RUSHDB_DEFAULT_RELATION_TYPE ?
      'Created automatically from your imported nested data (RushDB default relationship). Managed by the import structure, not editable here.'
    : 'Emerged from your imported data structure. Managed by the data itself, not editable here.'

  return (
    <li className="bg-card rounded-md border px-4 py-3">
      <div className="flex items-start justify-between gap-3">
        <div className="flex min-w-0 flex-col gap-1.5">
          <div className="flex flex-wrap items-center gap-3">
            <RelationshipPath
              labelNames={labelNames}
              sourceLabel={row.sourceLabel}
              targetLabel={row.targetLabel}
              type={row.type}
            />
            {applying ?
              <Badge className="text-sm text-content2">Applying…</Badge>
            : null}
          </div>
          {detail ?
            <p className="text-sm leading-snug text-content2">{detail}</p>
          : null}
        </div>
        {pattern ?
          <DeletePatternDialog
            loading={remove.isPending}
            onDelete={(deleteExisting) => remove.mutateAsync({ id: pattern.id, deleteExisting })}
            pattern={pattern}
          />
        : <Tooltip
            align="end"
            trigger={
              <span
                aria-label="Read-only relationship"
                className="inline-flex h-8 w-8 shrink-0 items-center justify-center text-content3"
              >
                <Lock className="h-4 w-4" />
              </span>
            }
          >
            <span className="max-w-[280px] text-sm">
              This relationship is part of the imported data structure and can't be edited here.
            </span>
          </Tooltip>
        }
      </div>
    </li>
  )
}

function RelationshipsOverviewSection({
  isPending,
  labelNames,
  schemaGraph,
  rows
}: {
  isPending: boolean
  labelNames: string[]
  schemaGraph: GraphOutput
  rows: RelationshipRowData[]
}) {
  const [view, setView] = useState<'list' | 'graph'>('list')

  return (
    <section className="flex flex-col gap-3">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold">Existing relationships</h2>
          <p className="text-sm text-content2">
            Review relationship types that already connect records in this project.
          </p>
        </div>
        <Tabs onValueChange={(value) => setView(value as 'list' | 'graph')} value={view}>
          <TabsList>
            <Tab value="list">
              <List className="h-4 w-4" />
              List
            </Tab>
            <Tab value="graph">
              <Network className="h-4 w-4" />
              Graph
            </Tab>
          </TabsList>
        </Tabs>
      </div>

      {isPending ?
        <Skeleton enabled>
          <Card className="h-28" />
        </Skeleton>
      : view === 'graph' ?
        schemaGraph.nodes.length ?
          <div className="h-[600px] overflow-hidden rounded-md border">
            <GraphCanvas
              availableLayers={['recordLabels', 'relationshipTypes']}
              getNodeHoverLabel={schemaNodeHoverLabel}
              graphData={schemaGraph}
            />
          </div>
        : <NothingFound title="No existing relationships found" />
      : rows.length ?
        <ul className="flex flex-col gap-3">
          {rows.map((row) => (
            <RelationshipRow key={row.key} labelNames={labelNames} row={row} />
          ))}
        </ul>
      : <NothingFound title="No existing relationships found" />}
    </section>
  )
}

export function PatternsSection({
  labelNames,
  loading,
  patterns,
  tourPatternId,
  selectable = false
}: {
  labelNames: string[]
  loading: boolean
  patterns: RelationshipPattern[]
  tourPatternId?: string
  selectable?: boolean
}) {
  const bulkApprove = useApproveRelationshipPatternsMutation()
  const [selectedIds, setSelectedIds] = useState<Set<string>>(() => new Set())

  // Drop selections for patterns that left this list (approved/ignored on refetch),
  // so the count and the bulk action never reference stale ids.
  useEffect(() => {
    setSelectedIds((prev) => {
      if (prev.size === 0) {
        return prev
      }
      const present = new Set(patterns.map((pattern) => pattern.id))
      const next = new Set([...prev].filter((id) => present.has(id)))
      return next.size === prev.size ? prev : next
    })
  }, [patterns])

  const toggleSelected = (id: string, checked: boolean) =>
    setSelectedIds((prev) => {
      const next = new Set(prev)
      if (checked) {
        next.add(id)
      } else {
        next.delete(id)
      }
      return next
    })

  const allSelected = patterns.length > 0 && selectedIds.size === patterns.length
  const toggleAll = (checked: boolean) =>
    setSelectedIds(checked ? new Set(patterns.map((pattern) => pattern.id)) : new Set())

  const approveSelected = () => {
    const ids = [...selectedIds]
    if (!ids.length) {
      return
    }
    bulkApprove.mutate(ids, { onSuccess: () => setSelectedIds(new Set()) })
  }

  if (loading) {
    return (
      <div className="flex flex-col gap-3">
        <Skeleton enabled>
          <Card className="h-32" />
        </Skeleton>
        <Skeleton enabled>
          <Card className="h-32" />
        </Skeleton>
      </div>
    )
  }

  if (!patterns.length) {
    return <NothingFound className="min-h-0 py-8" title="No patterns found" />
  }

  return (
    <div className="flex flex-col gap-3">
      {selectable ?
        <div className="bg-card flex min-h-14 items-center justify-between gap-3 rounded-md border px-4 py-2">
          <label className="flex items-center gap-2 text-sm text-content2">
            <Checkbox
              aria-label="Select all relationship patterns"
              checked={allSelected}
              onCheckedChange={(checked) => toggleAll(checked === true)}
            />
            {selectedIds.size > 0 ? `${selectedIds.size} selected` : 'Select all'}
          </label>
          {selectedIds.size > 0 ?
            <Button loading={bulkApprove.isPending} onClick={approveSelected} size="small" variant="primary">
              <Check />
              Approve selected ({selectedIds.size})
            </Button>
          : null}
        </div>
      : null}
      <ul className="flex flex-col gap-3">
        {patterns.map((pattern, index) => (
          <PatternCard
            key={pattern.id}
            labelNames={labelNames}
            onToggleSelected={toggleSelected}
            pattern={pattern}
            selectable={selectable}
            selected={selectedIds.has(pattern.id)}
            tourApproveTarget={pattern.id === tourPatternId || (!tourPatternId && index === 0)}
          />
        ))}
      </ul>
    </div>
  )
}

export function TabLabel({ count, label }: { count: number; label: string }) {
  return (
    <>
      <span>{label}</span>
      <span className="inline-flex h-5 min-w-5 items-center justify-center rounded-full bg-content3/10 px-1.5 text-sm text-content2">
        {count}
      </span>
    </>
  )
}

export function ProjectRelationships({ projectId }: { projectId: Project['id'] }) {
  void projectId
  const { data, isPending } = useRelationshipPatternsQuery()
  const patterns = data?.patterns ?? []
  const relationships = data?.relationships ?? []
  const approved = patterns.filter((pattern) => pattern.status === 'approved')
  const labelNames = useMemo(() => relationshipLabelNames(patterns, relationships), [patterns, relationships])
  const relationshipRows = useMemo(
    () => buildRelationshipRows(relationships, approved),
    [relationships, approved]
  )
  const schemaGraph = useMemo(
    () => buildSchemaGraph(relationshipRows, labelNames),
    [relationshipRows, labelNames]
  )

  return (
    <>
      <PageHeader className="items-start" contained>
        <div className="flex max-w-3xl flex-col gap-2">
          <PageTitle>Relationships</PageTitle>
          <p className="text-sm leading-6 text-content2">
            Relationships are named, directed edges between records. They turn imported data into a
            traversable graph and can be created directly, inferred from references, or approved from
            suggested patterns.
          </p>
          <a
            className="inline-flex w-fit items-center gap-2 text-sm text-content2 transition hover:text-content"
            href={RELATIONSHIP_DOCS_URL}
            rel="noreferrer"
            target="_blank"
          >
            Read the docs <ExternalLink className="h-3.5 w-3.5" />
          </a>
        </div>
      </PageHeader>
      <PageContent className="gap-8" contained>
        <RelationshipsOverviewSection
          isPending={isPending}
          labelNames={labelNames}
          schemaGraph={schemaGraph}
          rows={relationshipRows}
        />
      </PageContent>
    </>
  )
}
