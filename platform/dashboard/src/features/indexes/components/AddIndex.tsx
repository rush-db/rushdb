import { useQuery } from '@tanstack/react-query'
import { ChevronsUpDown } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'

import type { Property } from '@rushdb/javascript-sdk'
import type { EmbeddingIndex } from '~/features/indexes/types'
import type { Project } from '~/features/projects/types'

import { Button } from '~/elements/Button'
import { Card, CardBody, CardHeader } from '~/elements/Card'
import { Dialog, DialogTitle } from '~/elements/Dialog'
import { SearchSelect, SelectItem } from '~/elements/SearchSelect'
import { Skeleton } from '~/elements/Skeleton'
import { LabelName } from '~/features/labels/components/LabelName'
import { api } from '~/lib/api'
import { cn } from '~/lib/utils'

import { useAddIndexMutation } from '../hooks/useIndexMutations'
import { useProjectLabelsQuery } from '~/features/projects/hooks/useProjectQueries'

function useLabelPropertiesQuery(label: string) {
  return useQuery({
    queryKey: ['index-properties', label],
    queryFn: async () => {
      const result = await api.properties.find({
        searchQuery: { labels: [label] },
        init: {} as RequestInit
      })

      return result.data
        .filter((property) => property.type === 'string')
        .filter(
          (property, index, all) => all.findIndex((candidate) => candidate.name === property.name) === index
        )
    },
    enabled: !!label
  })
}

function isIndexed(existingIndexes: EmbeddingIndex[] | undefined, label: string, propertyName: string) {
  return (
    existingIndexes?.some(
      (index) =>
        index.sourceType === 'managed' && index.label === label && index.propertyName === propertyName
    ) ?? false
  )
}

export function AddIndexCard({
  existingIndexes,
  projectId
}: {
  existingIndexes?: EmbeddingIndex[]
  projectId: Project['id']
}) {
  return (
    <Card>
      <CardHeader title="Create embedding index" />
      <CardBody>
        <AddIndexForm existingIndexes={existingIndexes} projectId={projectId} />
      </CardBody>
    </Card>
  )
}

export function AddIndexDialog({
  existingIndexes,
  projectId,
  trigger
}: {
  existingIndexes?: EmbeddingIndex[]
  projectId: Project['id']
  trigger: React.ReactNode
}) {
  const [open, setOpen] = useState(false)

  return (
    <Dialog className="sm:max-w-2xl" onOpenChange={setOpen} open={open} trigger={trigger}>
      <DialogTitle>Create embedding index</DialogTitle>
      <p className="text-content2 mt-2">
        Choose a label and string property to make that field searchable by meaning. Managed indexes embed
        matching records and continue indexing new writes.
      </p>
      <AddIndexForm
        className="mt-6"
        existingIndexes={existingIndexes}
        onCreated={() => setOpen(false)}
        projectId={projectId}
      />
    </Dialog>
  )
}

function AddIndexForm({
  className,
  existingIndexes,
  onCreated,
  projectId
}: {
  className?: string
  existingIndexes?: EmbeddingIndex[]
  onCreated?: () => void
  projectId: Project['id']
}) {
  const { mutateAsync: mutate, error } = useAddIndexMutation()

  const { data: labels, isPending: loadingLabels } = useProjectLabelsQuery()

  const [label, setLabel] = useState<string>('')
  const [propertyName, setPropertyName] = useState<string>('')
  const [isSubmitting, setIsSubmitting] = useState(false)
  const {
    data: properties = [],
    isPending: propsPending,
    isFetching: propsFetching
  } = useLabelPropertiesQuery(label)
  const propsLoading = label.length > 0 && (propsPending || propsFetching)
  const availableProperties = useMemo(
    () => properties.filter((property) => !isIndexed(existingIndexes, label, property.name)),
    [existingIndexes, label, properties]
  )
  const labelEntries = useMemo(() => Object.entries(labels ?? {}), [labels])
  const selectedLabelIdx = useMemo(
    () => labelEntries.findIndex(([labelName]) => labelName === label),
    [labelEntries, label]
  )

  useEffect(() => {
    setPropertyName('')
  }, [label])

  useEffect(() => {
    if (propertyName && !availableProperties.some((property) => property.name === propertyName)) {
      setPropertyName('')
    }
  }, [availableProperties, propertyName])

  const isValid = label.length > 0 && propertyName.length > 0

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!isValid) return
    setIsSubmitting(true)
    try {
      await mutate({ projectId, label, propertyName })
      setLabel('')
      setPropertyName('')
      onCreated?.()
    } catch {
      // surfaced via the mutation's onError toast and the inline error below
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <form className={cn('flex flex-col gap-5', className)} onSubmit={handleSubmit}>
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <div className="flex flex-col gap-1.5">
          <span className="text-sm font-medium">Label</span>
          {loadingLabels ?
            <Skeleton enabled>
              <Button className="w-full" size="small" variant="outline">
                Loading…
              </Button>
            </Skeleton>
          : <SearchSelect
              trigger={
                <Button
                  className="w-full justify-between font-normal"
                  size="small"
                  type="button"
                  variant="outline"
                >
                  <span className={label ? '' : 'text-content3'}>
                    {label ?
                      <LabelName idx={Math.max(selectedLabelIdx, 0)} label={label} />
                    : 'Select a label'}
                  </span>
                  <ChevronsUpDown className="ml-2 h-4 w-4 shrink-0 opacity-50" />
                </Button>
              }
            >
              {labelEntries.map(([labelName], idx) => (
                <SelectItem key={labelName} onSelect={() => setLabel(labelName)} value={labelName}>
                  <LabelName idx={idx} label={labelName} />
                </SelectItem>
              ))}
            </SearchSelect>
          }
        </div>

        <div className="flex flex-col gap-1.5">
          <span className="text-sm font-medium">Property</span>
          {!label ?
            <Button
              className="w-full justify-between font-normal"
              disabled
              size="small"
              type="button"
              variant="outline"
            >
              <span className="text-content3">Select a label first</span>
              <ChevronsUpDown className="ml-2 h-4 w-4 shrink-0 opacity-50" />
            </Button>
          : propsLoading ?
            <Skeleton enabled>
              <Button className="w-full" size="small" variant="outline">
                Loading…
              </Button>
            </Skeleton>
          : <SearchSelect
              trigger={
                <Button
                  className="w-full justify-between font-normal"
                  size="small"
                  type="button"
                  variant="outline"
                >
                  <span className={propertyName ? '' : 'text-content3'}>
                    {propertyName || 'Select a text property'}
                  </span>
                  <ChevronsUpDown className="ml-2 h-4 w-4 shrink-0 opacity-50" />
                </Button>
              }
            >
              {availableProperties.length === 0 && label && !propsLoading ?
                <SelectItem disabled value="">
                  No text properties found
                </SelectItem>
              : availableProperties.map((property: Property) => (
                  <SelectItem
                    key={property.id}
                    onSelect={() => setPropertyName(property.name)}
                    value={property.name}
                  >
                    {property.name}
                  </SelectItem>
                ))
              }
            </SearchSelect>
          }
        </div>
      </div>

      {error != null && (
        <div className="text-sm text-red-500">
          {error instanceof Error ? error.message : 'Failed to create index'}
        </div>
      )}

      <div className="flex justify-end gap-2 border-t pt-5">
        <Button disabled={!isValid} loading={isSubmitting} type="submit" variant="accent">
          Create
        </Button>
      </div>
    </form>
  )
}
