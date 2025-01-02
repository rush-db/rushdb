import type { PropertyWithValue, Where, DBRecord } from '@rushdb/javascript-sdk'

import { getViableSearchOperations, type AnySearchOperation, type Filter } from '~/features/search/types'
import type { AnyObject } from '~/types'

import { SearchOperations } from '~/features/search/constants.ts'
import { removeProperty } from '~/lib/utils'

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
