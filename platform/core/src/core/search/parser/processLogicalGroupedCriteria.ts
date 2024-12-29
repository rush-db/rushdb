import { isEmptyObject } from '@/common/utils/isEmptyObject'
import { toBoolean } from '@/common/utils/toBolean'
import { DEFAULT_RECORD_ALIAS } from '@/core/common/constants'
import { Where } from '@/core/common/types'
import { parseCurrentLevel } from '@/core/search/parser/parseCurrentLevel'
import { ParseContext } from '@/core/search/parser/types'
import { isCurrentLevelCriteria, splitCriteria } from '@/core/search/parser/utils'
import { TSearchQueryBuilderOptions } from '@/core/search/search.types'

export const processLogicalGroupedCriteria = (
  key: string,
  input: Where,
  options?: TSearchQueryBuilderOptions,
  ctx?: ParseContext
) => {
  /** @description
   *
   * This function pulls out current level criteria for later usage
   * in WITH clause. Mixed queries (sub queries and current level criteria could be putted under
   * logical grouping. This is why querying them at the first WHERE is useless, as they should be checked
   * against existence of sub-queries. For example:
   *
   * $or: [
   *     {
   *         CAR: {
   *             color: 'red',
   *         },
   *     },
   *     {
   *         SPOUSE: {
   *             gender: 'male',
   *         },
   *     },
   *     {
   *         title: {
   *             $ne: 'Forest',
   *         },
   *     },
   * ],
   *
   * THE EXPECTED OUTPUT HERE:
   * {
   *     record: '(any(value IN record.created WHERE value = true)) AND (any(value IN record.rating WHERE value = 5))'
   *     record1: 'OPTIONAL MATCH (record)--(record1:CAR) WHERE (any(value IN record1.color WHERE value = "red"))'
   *     record2: 'OPTIONAL MATCH (record)--(record2:SPOUSE) WHERE (any(value IN record2.gender WHERE value = "male"))'
   * }
   *
   * SO THIS {record.title <> 'Forest'} clause will be checked lately along with existence of record1 OR record2
   *
   * */

  const { currentLevel, subQueries } = splitCriteria(input)

  const nextSubQueriesCount = Object.keys(subQueries).length

  if (nextSubQueriesCount && !isEmptyObject(currentLevel)) {
    ctx.withQueryQueue[DEFAULT_RECORD_ALIAS + nextSubQueriesCount] = [
      ...(ctx.withQueryQueue[options.nodeAlias] ?? []),
      ...parseCurrentLevel(key, currentLevel, options, ctx).flat().filter(toBoolean)
    ]
  }

  return parseCurrentLevel(key, isCurrentLevelCriteria(input) ? input : subQueries, options, ctx)
}
