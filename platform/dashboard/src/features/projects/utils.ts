import type { PropertyWithValue, Where, DBRecord } from '@rushdb/javascript-sdk'

import { getViableSearchOperations, type AnySearchOperation, type Filter } from '~/features/search/types'
import { AnyObject, Sort, SortDirection } from '~/types'

import { SearchOperations } from '~/features/search/constants.ts'
import { removeProperty } from '~/lib/utils'
import {
  $activeLabels,
  $combineFilters,
  $currentProjectFilters,
  $currentProjectRecordsLimit,
  $currentProjectRecordsSkip,
  $recordsOrderBy
} from '~/features/projects/stores/current-project.ts'
import { useStore } from '@nanostores/react'

export const decodeQuery = (query: string) => {
  return JSON.parse(decodeURIComponent(query))
}

export const encodeQuery = (object: AnyObject) => {
  return encodeURIComponent(JSON.stringify(object))
}

// TODO: move to another file, fix types
export const filterToSearchOperation = (filter: Filter) =>
  removeProperty(filter, 'filterId') as AnySearchOperation

export const collectPropertiesFromRecord = (record: DBRecord) => {
  return Object.entries(record ?? {})
    .map(([key, value]) => ({
      type: record?.__proptypes?.[key],
      name: key,
      value
    }))
    .filter(
      (prop) => prop.name !== '__proptypes' && prop.name !== '__id' && prop.name !== '__label'
    ) as Array<Pick<PropertyWithValue, 'name' | 'type' | 'value'>>
}

export const convertToSearchQuery = (filters: AnySearchOperation[] = []): Where<any> => {
  return filters?.reduce<Where<any>>((where, filter) => {
    const { name, operation, value } = filter

    const stringOperations = getViableSearchOperations('string')

    let convertedValue = value

    if (!stringOperations.includes(operation) && !Array.isArray(value)) {
      convertedValue = isNaN(Number(value)) ? value : Number(value)
    }

    switch (operation) {
      // @ts-expect-error
      case SearchOperations.Equals:
        // @ts-expect-error
        if (!where[name]) {
          // @ts-expect-error
          where[name] = value
          return where
        }

      default: {
        // @ts-expect-error
        if (!where[name]) where[name] = {}
        // @ts-expect-error
        where[name][operation] = convertedValue
        return where
      }
    }
  }, {})
}

export const isProjectEmpty = ({ loading, totalRecords }: { loading: boolean; totalRecords?: number }) => {
  if (loading || typeof totalRecords === 'undefined') {
    return false
  }

  return totalRecords < 1
}

export const useSearchQuery = () => {
  const filters = useStore($currentProjectFilters)
  const orderBy = useStore($recordsOrderBy)
  const skip = useStore($currentProjectRecordsSkip)
  const limit = useStore($currentProjectRecordsLimit)
  const labels = useStore($activeLabels)
  const combineMode = useStore($combineFilters)
  const properties = filters.map(filterToSearchOperation)

  const order = Object.entries(orderBy ?? {}).reduce<Sort>((acc, [key, direction]) => {
    if (key === '__id') {
      return direction as SortDirection
    }

    if (key && direction) {
      // @ts-ignore
      acc[key] = direction as SortDirection
    }
    return acc
  }, {})

  return {
    where:
      combineMode === 'or' ? { $or: convertToSearchQuery(properties) } : convertToSearchQuery(properties),
    orderBy: order,
    skip,
    limit,
    labels
  }
}
