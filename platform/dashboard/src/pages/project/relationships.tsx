import { Ban, Check, Info, RefreshCw, Trash2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useStore } from '@nanostores/react'

import type { Project } from '~/features/projects/types'
import type { ExistingRelationshipSummary, RelationshipPattern } from '~/features/relationship-patterns/types'

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
import { Tab, Tabs, TabsContent, TabsList } from '~/elements/Tabs'
import { Tooltip } from '~/elements/Tooltip'
import {
  useAnalyzeRelationshipPatternsMutation,
  useApproveRelationshipPatternMutation,
  useDeleteRelationshipPatternMutation,
  useIgnoreRelationshipPatternMutation,
  useRelationshipPatternsQuery
} from '~/features/relationship-patterns/hooks'
import { $currentProjectId } from '~/features/projects/stores/id'
import { $tourStep, setTourStep } from '~/features/tour/stores/tour'
import { formatIsoToLocalDateTime } from '~/lib/formatters'
import { changeSearchParam, openRoute } from '~/lib/router'
import { cn } from '~/lib/utils'

function RelationshipPath({
  sourceLabel,
  sourceKey,
  targetLabel,
  targetKey,
  type
}: {
  sourceLabel: string
  sourceKey?: string
  targetLabel: string
  targetKey?: string
  type: string
}) {
  return (
    <div className="flex flex-wrap items-center gap-2 font-mono text-sm">
      <Label className="!text-sm">
        {sourceLabel}
        {sourceKey ? `[${sourceKey}]` : ''}
      </Label>
      <span className="text-content3">→</span>
      <Badge className="!text-sm">{type}</Badge>
      <span className="text-content3">→</span>
      <Label className="!text-sm">
        {targetLabel}
        {targetKey ? `[${targetKey}]` : ''}
      </Label>
    </div>
  )
}

function PatternPath({ pattern }: { pattern: RelationshipPattern }) {
  return (
    <div className="flex flex-wrap items-center gap-3">
      <RelationshipPath
        sourceLabel={pattern.source.label}
        sourceKey={pattern.source.key}
        targetLabel={pattern.target.label}
        targetKey={pattern.target.key}
        type={pattern.type}
      />
      <Badge className="text-content2 text-sm">
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

function isAgentRunJoinPattern(pattern: RelationshipPattern) {
  const labels = [pattern.source.label, pattern.target.label].map((label) => label.toUpperCase())
  const keys = [pattern.source.key, pattern.target.key].map((key) => key?.toLowerCase())

  return (
    pattern.mode === 'join_pattern' &&
    labels.some((label) => label.includes('AGENT')) &&
    labels.some((label) => label.includes('RUN')) &&
    keys.includes('agentid')
  )
}

function sortSuggestedPatterns(patterns: RelationshipPattern[]) {
  return [...patterns].sort((a, b) => {
    const aScore = isAgentRunJoinPattern(a) ? 1 : 0
    const bScore = isAgentRunJoinPattern(b) ? 1 : 0
    if (bScore !== aScore) return bScore - aScore
    return b.confidence - a.confidence
  })
}

function PatternCard({
  pattern,
  tourApproveTarget = false
}: {
  pattern: RelationshipPattern
  tourApproveTarget?: boolean
}) {
  const approve = useApproveRelationshipPatternMutation()
  const ignore = useIgnoreRelationshipPatternMutation()
  const remove = useDeleteRelationshipPatternMutation()
  const projectId = useStore($currentProjectId)
  const tourStep = useStore($tourStep)

  return (
    <li className="px-4 py-3">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <PatternPath pattern={pattern} />
        <div className="flex shrink-0 flex-wrap items-center justify-end gap-3">
          {pattern.status === 'suggested' || pattern.status === 'ignored' || pattern.status === 'error' ?
            <div className="text-content2 flex items-center gap-1 text-sm">
              <span>{Math.round(pattern.confidence * 100)}%</span>
              <Tooltip
                align="end"
                trigger={
                  <button
                    aria-label="Confidence score details"
                    className="text-content3 hover:text-content inline-flex h-5 w-5 items-center justify-center"
                    type="button"
                  >
                    <Info size={14} />
                  </button>
                }
              >
                <span className="font-semibold">Confidence Score</span>
                {pattern.rationale ?
                  <span className="max-w-[320px] text-sm">{pattern.rationale}</span>
                : null}
                {pattern.lastError ?
                  <span className="text-danger max-w-[320px] text-sm">{pattern.lastError}</span>
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
                variant="accent"
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

function ExistingRelationships({ relationships }: { relationships: ExistingRelationshipSummary[] }) {
  if (!relationships.length) {
    return <NothingFound title="No existing relationships found" />
  }

  return (
    <Card>
      <ul className="divide-y">
        {relationships.map((item) =>
          item.relationships.map((relationship) => {
            const sourceLabel = relationship.direction === 'in' ? relationship.label : item.label
            const targetLabel = relationship.direction === 'in' ? item.label : relationship.label

            return (
              <li
                className="px-4 py-3"
                key={`${item.label}-${relationship.direction}-${relationship.type}-${relationship.label}`}
              >
                <RelationshipPath
                  sourceLabel={sourceLabel}
                  targetLabel={targetLabel}
                  type={relationship.type}
                />
              </li>
            )
          })
        )}
      </ul>
    </Card>
  )
}

function ExistingRelationshipsSection({
  isPending,
  relationships
}: {
  isPending: boolean
  relationships: ExistingRelationshipSummary[]
}) {
  return (
    <section className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2 className="text-lg font-semibold">Existing relationships</h2>
      </div>
      {isPending ?
        <Skeleton enabled>
          <Card className="h-28" />
        </Skeleton>
      : <ExistingRelationships relationships={relationships} />}
    </section>
  )
}

function PatternsSection({
  loading,
  patterns,
  tourPatternId
}: {
  loading: boolean
  patterns: RelationshipPattern[]
  tourPatternId?: string
}) {
  return (
    <>
      {loading ?
        <div className="flex flex-col gap-3">
          <Skeleton enabled>
            <Card className="h-32" />
          </Skeleton>
          <Skeleton enabled>
            <Card className="h-32" />
          </Skeleton>
        </div>
      : patterns.length ?
        <Card>
          <ul className="divide-y">
            {patterns.map((pattern, index) => (
              <PatternCard
                key={pattern.id}
                pattern={pattern}
                tourApproveTarget={pattern.id === tourPatternId || (!tourPatternId && index === 0)}
              />
            ))}
          </ul>
        </Card>
      : <NothingFound className="min-h-0 py-8" title="No patterns found" />}
    </>
  )
}

function TabLabel({ count, label }: { count: number; label: string }) {
  return (
    <>
      <span>{label}</span>
      <span className="bg-content3/10 text-content2 inline-flex h-5 min-w-5 items-center justify-center rounded-full px-1.5 text-sm">
        {count}
      </span>
    </>
  )
}

export function ProjectRelationships({ projectId }: { projectId: Project['id'] }) {
  void projectId
  const { data, isPending } = useRelationshipPatternsQuery()
  const analyze = useAnalyzeRelationshipPatternsMutation()
  const patterns = data?.patterns ?? []
  const suggested = sortSuggestedPatterns(patterns.filter((pattern) => pattern.status === 'suggested'))
  const approved = patterns.filter((pattern) => pattern.status === 'approved')
  const ignored = patterns.filter((pattern) => pattern.status === 'ignored' || pattern.status === 'error')
  const lastAnalyzedAt = data?.analysis?.lastRunAt
  const isAnalyzing =
    analyze.isPending || data?.analysis?.status === 'pending' || data?.analysis?.status === 'running'
  const tourStep = useStore($tourStep)
  const tourPatternId = suggested.find(isAgentRunJoinPattern)?.id ?? suggested[0]?.id

  useEffect(() => {
    if (tourStep === 'projectRelationshipAnalyze' && !isAnalyzing && suggested.length > 0) {
      setTourStep('projectRelationshipApprove', true)
    }
  }, [isAnalyzing, suggested.length, tourStep])

  return (
    <>
      <PageHeader contained>
        <div className="flex flex-col gap-2">
          <PageTitle>Relationships</PageTitle>
        </div>
      </PageHeader>
      <PageContent contained>
        <ExistingRelationshipsSection isPending={isPending} relationships={data?.relationships ?? []} />

        <section className="flex flex-col gap-3">
          <div className="flex flex-col gap-1">
            <h2 className="text-lg font-semibold">Suggested relationships</h2>
            <p className="text-content2 max-w-3xl text-sm">
              RushDB analyzes your ontology after writes and suggests relationship patterns between records
              that look connected. Some patterns match reference fields; others rename imported default
              relationships when nested data already created the structure. Approving a pattern applies it now
              and keeps applying it to future writes.
            </p>
          </div>

          <Tabs className="flex flex-col gap-3" defaultValue="suggested">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <TabsList>
                <Tab value="suggested">
                  <TabLabel count={suggested.length} label="Suggested" />
                </Tab>
                <Tab value="approved">
                  <TabLabel count={approved.length} label="Approved" />
                </Tab>
                <Tab value="ignored">
                  <TabLabel count={ignored.length} label="Ignored" />
                </Tab>
              </TabsList>
              <div className="flex flex-wrap items-center justify-end gap-3">
                {isAnalyzing ?
                  <span className="text-content2 text-sm">Exploring graph...</span>
                : lastAnalyzedAt ?
                  <span className="text-content2 text-sm" title={lastAnalyzedAt}>
                    Last analyzed {formatIsoToLocalDateTime(lastAnalyzedAt)}
                  </span>
                : null}
                <Button
                  data-tour="project-relationships-analyze"
                  disabled={isAnalyzing}
                  onClick={() => analyze.mutate()}
                  size="small"
                  variant="secondary"
                >
                  <RefreshCw className={cn(isAnalyzing && 'animate-spin')} />
                  Refresh
                </Button>
              </div>
            </div>
            <TabsContent value="suggested">
              <PatternsSection loading={isPending} patterns={suggested} tourPatternId={tourPatternId} />
            </TabsContent>
            <TabsContent value="approved">
              <PatternsSection loading={isPending} patterns={approved} />
            </TabsContent>
            <TabsContent value="ignored">
              <PatternsSection loading={isPending} patterns={ignored} />
            </TabsContent>
          </Tabs>
        </section>
      </PageContent>
    </>
  )
}
