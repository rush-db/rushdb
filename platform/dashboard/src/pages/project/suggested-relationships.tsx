import { ExternalLink, RefreshCw } from 'lucide-react'
import { useEffect, useMemo } from 'react'
import { useStore } from '@nanostores/react'

import type { Project } from '~/features/projects/types'

import { Button } from '~/elements/Button'
import { NothingFound } from '~/elements/NothingFound'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { Tab, Tabs, TabsContent, TabsList } from '~/elements/Tabs'
import {
  useAnalyzeRelationshipPatternsMutation,
  useRelationshipPatternsQuery
} from '~/features/relationship-patterns/hooks'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { $tourStep, setTourStep } from '~/features/tour/stores/tour'
import { formatIsoToLocalDateTime } from '~/lib/formatters'
import { cn } from '~/lib/utils'

import {
  isAgentRunJoinPattern,
  PatternsSection,
  relationshipLabelNames,
  sortSuggestedPatterns,
  SUGGESTED_PATTERNS_DOCS_URL,
  TabLabel
} from '~/pages/project/relationships'

export function ProjectSuggestedRelationships({ projectId }: { projectId: Project['id'] }) {
  void projectId
  const { data, isPending } = useRelationshipPatternsQuery()
  const { data: platformSettings, isPending: settingsPending } = usePlatformSettings()
  const analyze = useAnalyzeRelationshipPatternsMutation()
  const llmEnabled = platformSettings?.llmEnabled === true

  const patterns = data?.patterns ?? []
  const relationships = data?.relationships ?? []
  const suggested = sortSuggestedPatterns(patterns.filter((pattern) => pattern.status === 'suggested'))
  const ignored = patterns.filter((pattern) => pattern.status === 'ignored' || pattern.status === 'error')
  const labelNames = useMemo(() => relationshipLabelNames(patterns, relationships), [patterns, relationships])

  const lastAnalyzedAt = data?.analysis?.lastRunAt
  const isAnalyzing =
    analyze.isPending || data?.analysis?.status === 'pending' || data?.analysis?.status === 'running'
  const tourStep = useStore($tourStep)
  const tourPatternId = suggested.find(isAgentRunJoinPattern)?.id ?? suggested[0]?.id

  useEffect(() => {
    if (llmEnabled && tourStep === 'projectRelationshipAnalyze' && !isAnalyzing && suggested.length > 0) {
      setTourStep('projectRelationshipApprove', true)
    }
  }, [isAnalyzing, llmEnabled, suggested.length, tourStep])

  return (
    <>
      <PageHeader className="items-start" contained>
        <div className="flex max-w-3xl flex-col gap-2">
          <PageTitle>Suggested Relationships</PageTitle>
          <p className="text-content2 text-sm leading-6">
            RushDB analyzes this project after writes and suggests relationship patterns worth reviewing.
            Approving a pattern applies it now and keeps applying it to matching future writes.
          </p>
          <a
            className="text-content2 hover:text-content inline-flex w-fit items-center gap-2 text-sm transition"
            href={SUGGESTED_PATTERNS_DOCS_URL}
            rel="noreferrer"
            target="_blank"
          >
            Read the docs <ExternalLink className="h-3.5 w-3.5" />
          </a>
        </div>
      </PageHeader>
      <PageContent className="gap-8" contained>
        {settingsPending || llmEnabled ?
          <Tabs className="flex flex-col gap-3" defaultValue="suggested">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <TabsList>
                <Tab value="suggested">
                  <TabLabel count={suggested.length} label="Suggested" />
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
                  disabled={isAnalyzing || !llmEnabled}
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
              <PatternsSection
                labelNames={labelNames}
                loading={isPending || settingsPending}
                patterns={suggested}
                selectable
                tourPatternId={tourPatternId}
              />
            </TabsContent>
            <TabsContent value="ignored">
              <PatternsSection
                labelNames={labelNames}
                loading={isPending || settingsPending}
                patterns={ignored}
              />
            </TabsContent>
          </Tabs>
        : <NothingFound title="AI relationship suggestions are not configured" />}
      </PageContent>
    </>
  )
}
