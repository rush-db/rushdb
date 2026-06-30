import { useQuery } from '@tanstack/react-query'
import { useStore } from '@nanostores/react'
import { Lightbulb, Plus } from 'lucide-react'

import type { EmbeddingIndex } from '~/features/indexes/types'
import type { Project } from '~/features/projects/types'

import { Button } from '~/elements/Button'
import { Label } from '~/elements/Label'
import { Skeleton } from '~/elements/Skeleton'
import { useAddIndexMutation } from '~/features/indexes/hooks/useIndexMutations'
import {
  buildSuggestedEmbeddingIndexes,
  flattenSchemaProperties,
  type SuggestedEmbeddingIndex
} from '~/features/indexes/utils/suggestedIndexes'
import { getLabelColor } from '~/features/labels'
import { $tourStep, setTourStep } from '~/features/tour/stores/tour'
import { api } from '~/lib/api'
import { openRoute } from '~/lib/router'

function useSuggestedIndexesQuery({
  existingIndexes,
  projectId
}: {
  existingIndexes?: EmbeddingIndex[]
  projectId: Project['id']
}) {
  const schemaQuery = useQuery({
    queryKey: ['projects', projectId, 'suggested-index-schema'],
    queryFn: async ({ signal }) => {
      return api.ai.getSchema({
        projectId,
        init: { signal } as RequestInit
      })
    },
    enabled: !!projectId
  })

  const schema = schemaQuery.data ?? []
  const labelEntries = schema.map((item) => [item.label, item.count] as const)
  const suggestions = buildSuggestedEmbeddingIndexes({
    existingIndexes,
    properties: flattenSchemaProperties(schema)
  })

  return {
    isPending: schemaQuery.isPending,
    labelEntries,
    suggestions
  }
}

function SuggestedIndexItem({
  projectId,
  suggestion,
  labelIdx,
  primaryTourTarget
}: {
  labelIdx: number
  primaryTourTarget: boolean
  projectId: Project['id']
  suggestion: SuggestedEmbeddingIndex
}) {
  const { isPending, mutateAsync } = useAddIndexMutation()
  const tourStep = useStore($tourStep)

  return (
    <li className="bg-card flex flex-col gap-3 rounded-md border px-3 py-3 sm:flex-row sm:items-center sm:justify-between sm:gap-4 sm:px-4">
      <div className="flex min-w-0 flex-col gap-2">
        <div className="flex flex-wrap items-center gap-2 text-sm">
          <Label className="!text-sm" variant={getLabelColor(suggestion.label, Math.max(labelIdx, 0))}>
            {suggestion.label}
          </Label>
          <span className="text-content3">:</span>
          <span className="bg-content3/10 text-content2 w-fit rounded-sm px-1 font-mono text-xs">
            {suggestion.propertyName}
          </span>
        </div>
        <p className="text-content2 text-sm">
          {suggestion.reason}. Create a semantic index to search it by meaning.
        </p>
      </div>
      <div className="flex items-center justify-end gap-3 sm:shrink-0">
        <span className="text-content3 whitespace-nowrap text-sm tabular-nums">
          {(suggestion.recordsCount ?? 0).toLocaleString()} records
        </span>
        <Button
          data-tour={primaryTourTarget ? 'project-index-suggested-create' : undefined}
          onClick={() =>
            mutateAsync({
              projectId,
              label: suggestion.label,
              propertyName: suggestion.propertyName
            })
              .then(() => {
                if (tourStep === 'projectIndexCreate') {
                  openRoute('projectRelationships', { id: projectId })
                  setTourStep('projectRelationshipAnalyze', true)
                }
              })
              .catch(() => {
                // surfaced via the mutation's onError toast
              })
          }
          loading={isPending}
          size="small"
          variant="primary"
        >
          <Plus />
          Create Index
        </Button>
      </div>
    </li>
  )
}

export function SuggestedIndexesCard({
  existingIndexes,
  indexesLoading,
  projectId
}: {
  existingIndexes?: EmbeddingIndex[]
  indexesLoading: boolean
  projectId: Project['id']
}) {
  const { isPending, labelEntries, suggestions } = useSuggestedIndexesQuery({ existingIndexes, projectId })
  const loading = indexesLoading || isPending

  if (!loading && suggestions.length === 0) {
    return null
  }

  const description =
    !loading && suggestions.length > 0 ?
      `RushDB found ${suggestions.length} text fields that are likely useful for semantic search.`
    : 'RushDB found text fields that are likely useful for semantic search.'

  return (
    <div className="flex flex-col gap-3" data-tour="project-index-suggestions">
      <div>
        <h2 className="text-content flex items-center gap-2 text-lg font-semibold">
          <Lightbulb className="text-accent h-5 w-5" />
          Suggested semantic indexes
        </h2>
        <p className="text-content2 text-sm">{description}</p>
      </div>
      {loading ?
        <div className="flex flex-col gap-3">
          <Skeleton enabled>
            <div className="h-16 rounded-md border" />
          </Skeleton>
          <Skeleton enabled>
            <div className="h-16 rounded-md border" />
          </Skeleton>
        </div>
      : <ul className="flex flex-col gap-3">
          {suggestions.map((suggestion, idx) => (
            <SuggestedIndexItem
              key={`${suggestion.label}:${suggestion.propertyName}`}
              labelIdx={labelEntries.findIndex(([label]) => label === suggestion.label)}
              primaryTourTarget={idx === 0}
              projectId={projectId}
              suggestion={suggestion}
            />
          ))}
        </ul>
      }
    </div>
  )
}
