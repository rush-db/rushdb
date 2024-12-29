import { SORT_ASC, SORT_DESC } from '@/core/search/search.constants'

export type TSearchSortDirection = typeof SORT_ASC | typeof SORT_DESC
export type TSearchSortMap = Record<string, TSearchSortDirection>
export type TSearchSort = TSearchSortMap | TSearchSortDirection

export type TSearchQueryBuilderOptions = {
  nodeAlias: string
  joinOperator?: string
}
