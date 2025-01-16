import { toBoolean } from '@/common/utils/toBolean'
import { DEFAULT_RECORD_ALIAS } from '@/core/common/constants'
import { SearchDto } from '@/core/search/dto/search.dto'
import { buildOrderByClause } from '@/core/search/parser/orderBy'
import { pagination } from '@/core/search/parser/pagination'
import { parse } from '@/core/search/parser/parse'
import { TSearchSort } from '@/core/search/search.types'

export const buildLabelsClause = (labels?: string[]): string => {
  if (!labels || labels.length <= 1) {
    return ''
  }
  const labelsCriteria = labels
    .filter(toBoolean)
    .map((label) => `"${label}"`)
    .join(', ')

  if (!labelsCriteria) {
    return ''
  }
  return `(any(label in labels(record) WHERE label IN [${labelsCriteria}]))`
}

const sortQueryParts = (queryParts: Record<string, string>): string[] => {
  // @FYI: We sort the keys in ascending order
  // because the optional matches are generated in reverse order.
  // This is due to parsing the query from the deepest level (nested)
  // up to the root level.
  return Object.keys(queryParts)
    .sort((a, b) => a.localeCompare(b))
    .map((key) => queryParts[key])
}

export function buildPagination({ skip: skipRaw = 0, limit: limitRaw = 100 }: SearchDto) {
  const { skip, limit } = pagination(skipRaw, limitRaw)
  return `SKIP ${skip} LIMIT ${limit}`
}

export function sort(orderBy: TSearchSort, alias = DEFAULT_RECORD_ALIAS) {
  const orderClauses = buildOrderByClause(orderBy, alias)
  return `ORDER BY ${orderClauses.join(', ')}`
}

export const buildQueryClause = ({
  queryParts,
  labelClause,
  sortParams,
  pagination,
  hasRelatedConditions
}: {
  queryParts: string[]
  labelClause?: string
  sortParams: string
  pagination: string
  hasRelatedConditions: boolean
}) => {
  return queryParts
    .map((part, index) => {
      if (index === 0) {
        const whereClause = toBoolean(part) ? `WHERE ${part}` : ''

        const combinedClause = `${whereClause}${
          labelClause ? (whereClause ? ' AND ' : 'WHERE ') + labelClause : ''
        }`
        return `${combinedClause} ${sortParams} ${hasRelatedConditions ? '' : pagination}`.trim()
      }
      return part
    })
    .join('\n')
}

export const buildQuery = (searchParams: SearchDto) => {
  const parsedWhere = parse(searchParams.where)

  const hasRelatedConditions = parsedWhere.nodeAliases.length > 1

  // Sort query parts in ascending order
  const sortedQueryParts = sortQueryParts(parsedWhere.queryParts)

  const pagination = buildPagination(searchParams)
  const sortParams = sort(searchParams.orderBy)

  const queryClauses = buildQueryClause({
    queryParts: sortedQueryParts,
    pagination,
    sortParams,
    labelClause: buildLabelsClause(searchParams.labels),
    hasRelatedConditions
  })

  return {
    queryClauses,
    sortedQueryParts,
    aliasesMap: parsedWhere.aliasesMap,
    pagination,
    hasRelatedConditions,
    parsedWhere
  }
}
