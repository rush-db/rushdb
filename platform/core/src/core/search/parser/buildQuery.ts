import { isArray } from '@/common/utils/isArray'
import { toBoolean } from '@/common/utils/toBolean'
import { DEFAULT_RECORD_ALIAS } from '@/core/common/constants'
import { Where } from '@/core/common/types'
import { SearchDto } from '@/core/search/dto/search.dto'
import { buildWhereClause } from '@/core/search/parser/buildWhereClause'
import { buildOrderByClause } from '@/core/search/parser/orderBy'
import { pagination } from '@/core/search/parser/pagination'
import { parseCurrentLevel } from '@/core/search/parser/parseCurrentLevel'
import { parseSubQuery } from '@/core/search/parser/parseSubQuery'
import { processCriteria } from '@/core/search/parser/processCriteria'
import { ParseContext } from '@/core/search/parser/types'
import { splitCriteria } from '@/core/search/parser/utils'
import { TSearchQueryBuilderOptions, TSearchSort } from '@/core/search/search.types'

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
  pagination
}: {
  queryParts: string[]
  labelClause?: string
  sortParams: string
  pagination: string
}) => {
  return queryParts
    .map((part, index) => {
      if (index === 0) {
        const whereClause = toBoolean(part) ? `WHERE ${part}` : ''

        const combinedClause = `${whereClause}${
          labelClause ? (whereClause ? ' AND ' : 'WHERE ') + labelClause : ''
        }`
        return `${combinedClause} ${sortParams} ${pagination}`.trim()
      }
      return part
    })
    .join('\n')
}

const parse = (input: Where, options: TSearchQueryBuilderOptions = { nodeAlias: DEFAULT_RECORD_ALIAS }) => {
  const normalizedInput = processCriteria(input)

  const nodeAliases = [options.nodeAlias]
  const aliasesMap = { $record: options.nodeAlias }
  const ctx: ParseContext = {
    nodeAliases,
    aliasesMap,
    level: 0,
    result: { [options.nodeAlias]: '' },
    withQueryQueue: { [options.nodeAlias]: [] }
  }

  parseLevel('', normalizedInput, options, ctx)

  return {
    queryParts: ctx.result,
    nodeAliases,
    aliasesMap,
    where: buildWhereClause(normalizedInput, options, ctx)
  }
}

export const parseLevel = (
  key: string,
  input: Where,
  options?: TSearchQueryBuilderOptions,
  ctx?: ParseContext
) => {
  const { currentLevel, subQueries } = splitCriteria(input)

  // SUB QUERY PROCESSING
  if (toBoolean(subQueries)) {
    Object.entries(subQueries).forEach(([k, value]) => parseSubQuery(k, value as Where, options, ctx))
  }

  let result = ''

  if (toBoolean(currentLevel)) {
    result = parseCurrentLevel(key, currentLevel, options, ctx)

    // @TODO: Parenthesis ??? (...)
    const queryPart = isArray(result) ? result.filter(toBoolean).join(' AND ') : result
    ctx.result[options.nodeAlias] = (ctx.result[options.nodeAlias] ?? '') + queryPart
  }

  return result
}

export const buildQuery = (searchQuery: SearchDto) => {
  const parsedWhere = parse(searchQuery.where)

  // Sort query parts in ascending order
  const sortedQueryParts = sortQueryParts(parsedWhere.queryParts)

  const pagination = buildPagination(searchQuery)
  const sortParams = sort(searchQuery.orderBy)

  const queryClauses = buildQueryClause({
    queryParts: sortedQueryParts,
    pagination,
    sortParams,
    labelClause: buildLabelsClause(searchQuery.labels)
  })

  return {
    queryClauses,
    sortedQueryParts,
    aliasesMap: parsedWhere.aliasesMap,
    parsedWhere
  }
}
