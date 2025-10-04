import { toBoolean } from '@/common/utils/toBolean'
import { RUSHDB_KEY_ID, ROOT_RECORD_ALIAS } from '@/core/common/constants'

import { SORT_DESC, SORT_ASC } from '../search.constants'
import { TSearchSort, TSearchSortMap } from '../search.types'

export const buildSortCriteria = (orderBy: TSearchSort) => {
  let sortCriteria: TSearchSortMap

  if (!toBoolean(orderBy)) {
    sortCriteria = { [RUSHDB_KEY_ID]: SORT_DESC }
  } else if (typeof orderBy === 'string' && (orderBy === SORT_ASC || orderBy === SORT_DESC)) {
    sortCriteria = { [RUSHDB_KEY_ID]: orderBy }
  } else {
    sortCriteria = orderBy
  }

  return sortCriteria
}

export const buildOrderByClause = (orderBy: TSearchSort, alias: string | null = ROOT_RECORD_ALIAS) => {
  const sortCriteria = buildSortCriteria(orderBy)

  return Object.entries(sortCriteria).map(([property, direction]) => {
    if (alias === null) {
      return `\`${property}\` ${direction.toUpperCase()}`
    }

    return `${alias}.\`${property}\` ${direction.toUpperCase()}`
  })
}
