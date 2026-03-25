import { useQuery } from '@tanstack/react-query'
import { ChevronsUpDown } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'

import type { Property } from '@rushdb/javascript-sdk'
import type { Project } from '~/features/projects/types'

import { Button } from '~/elements/Button'
import { Card, CardBody, CardFooter, CardHeader } from '~/elements/Card'
import { SearchSelect, SelectItem } from '~/elements/SearchSelect'
import { Skeleton } from '~/elements/Skeleton'
import { LabelName } from '~/features/labels/components/LabelName'
import { api } from '~/lib/api'

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

export function AddIndexCard({ projectId }: { projectId: Project['id'] }) {
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
  const labelEntries = useMemo(() => Object.entries(labels ?? {}), [labels])
  const selectedLabelIdx = useMemo(
    () => labelEntries.findIndex(([labelName]) => labelName === label),
    [labelEntries, label]
  )

  useEffect(() => {
    setPropertyName('')
  }, [label])

  const isValid = label.length > 0 && propertyName.length > 0

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!isValid) return
    setIsSubmitting(true)
    try {
      await mutate({ projectId, label, propertyName })
      setLabel('')
      setPropertyName('')
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <Card>
      <form onSubmit={handleSubmit}>
        <CardHeader title="Create embedding index" />
        <CardBody className="grid grid-cols-1 gap-4 sm:grid-cols-2">
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
                {properties.length === 0 && label && !propsLoading ?
                  <SelectItem disabled value="">
                    No text properties found
                  </SelectItem>
                : properties.map((property: Property) => (
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
        </CardBody>

        {error != null && (
          <div className="px-4 pb-2 text-sm text-red-500">
            {error instanceof Error ? error.message : 'Failed to create index'}
          </div>
        )}

        <CardFooter className="mt-5">
          <Button disabled={!isValid} loading={isSubmitting} type="submit" variant="accent">
            Create
          </Button>
        </CardFooter>
      </form>
    </Card>
  )
}
