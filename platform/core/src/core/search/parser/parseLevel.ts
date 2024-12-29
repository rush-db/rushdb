import { isArray } from '@/common/utils/isArray'
import { toBoolean } from '@/common/utils/toBolean'
import { Where } from '@/core/common/types'
import { parseCurrentLevel } from '@/core/search/parser/parseCurrentLevel'
import { parseSubQuery } from '@/core/search/parser/parseSubQuery'
import { ParseContext } from '@/core/search/parser/types'
import { splitCriteria } from '@/core/search/parser/utils'
import { TSearchQueryBuilderOptions } from '@/core/search/search.types'

export const parseLevel = (
  key: string,
  input: Where,
  options?: TSearchQueryBuilderOptions,
  ctx?: ParseContext
) => {
  const { currentLevel, subQueries } = splitCriteria(input)

  // SUB QUERY PROCESSING
  if (toBoolean(subQueries)) {
    Object.entries(subQueries).forEach(([k, value]) =>
      parseSubQuery(k, value as Where, options, ctx)
    )
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
