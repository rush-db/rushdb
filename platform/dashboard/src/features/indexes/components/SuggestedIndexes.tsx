import { useQuery } from '@tanstack/react-query'
import { useStore } from '@nanostores/react'
import { Lightbulb, Search } from 'lucide-react'

import type { EmbeddingIndex } from '~/features/indexes/types'
import type { Project } from '~/features/projects/types'

import { Badge } from '~/elements/Badge'
import { Button } from '~/elements/Button'
import { Card, CardBody, CardHeader } from '~/elements/Card'
import { Label } from '~/elements/Label'
import { Skeleton } from '~/elements/Skeleton'
import { useAddIndexMutation } from '~/features/indexes/hooks/useIndexMutations'
import {
  buildSuggestedEmbeddingIndexes,
  flattenOntologyProperties,
  type SuggestedEmbeddingIndex
} from '~/features/indexes/utils/suggestedIndexes'
import { LabelName } from '~/features/labels/components/LabelName'
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
  const ontologyQuery = useQuery({
    queryKey: ['projects', projectId, 'suggested-index-ontology'],
    queryFn: async ({ signal }) => {
      return api.ai.getOntology({
        projectId,
        init: { signal } as RequestInit
      })
    },
    enabled: !!projectId
  })

  const ontology = ontologyQuery.data ?? []
  const labelEntries = ontology.map((item) => [item.label, item.count] as const)
  const suggestions = buildSuggestedEmbeddingIndexes({
    existingIndexes,
    properties: flattenOntologyProperties(ontology)
  })

  return {
    isPending: ontologyQuery.isPending,
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
    <li className="flex flex-col gap-3 border-t px-4 py-4 first:border-t-0 sm:flex-row sm:items-center sm:justify-between">
      <div className="flex min-w-0 flex-col gap-2">
        <div className="flex flex-wrap items-center gap-2">
          <LabelName idx={Math.max(labelIdx, 0)} label={suggestion.label} />
          <span className="text-content3">:</span>
          <Label>{suggestion.propertyName}</Label>
          <Badge>{suggestion.recordsCount ?? 0} records</Badge>
        </div>
        <p className="text-content2 text-sm">
          {suggestion.reason}. Create an embedding index for semantic search.
        </p>
      </div>
      <Button
        data-tour={primaryTourTarget ? 'project-index-suggested-create' : undefined}
        onClick={() =>
          mutateAsync({
            projectId,
            label: suggestion.label,
            propertyName: suggestion.propertyName
          }).then(() => {
            if (tourStep === 'projectIndexCreate') {
              openRoute('projectRelationships', { id: projectId })
              setTourStep('projectRelationshipAnalyze', true)
            }
          })
        }
        loading={isPending}
        size="small"
        variant="accent"
      >
        <Search />
        Create Index
      </Button>
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
  const visibleSuggestions = suggestions.slice(0, 4)

  if (!loading && visibleSuggestions.length === 0) {
    return null
  }

  return (
    <Card data-tour="project-index-suggestions">
      <CardHeader
        title={
          <span className="flex items-center gap-2">
            <Lightbulb className="text-accent h-5 w-5" />
            Suggested embedding indexes
          </span>
        }
        description="RushDB found text fields that are likely useful for semantic search."
      />
      <CardBody className="p-0">
        {loading ?
          <div className="flex flex-col gap-3 px-4 pb-4">
            <Skeleton enabled>
              <div className="h-16 rounded-md border" />
            </Skeleton>
            <Skeleton enabled>
              <div className="h-16 rounded-md border" />
            </Skeleton>
          </div>
        : <ul className="flex flex-col">
            {visibleSuggestions.map((suggestion, idx) => (
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
      </CardBody>
    </Card>
  )
}
