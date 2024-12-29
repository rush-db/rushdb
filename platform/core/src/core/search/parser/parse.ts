import { DEFAULT_RECORD_ALIAS } from '@/core/common/constants'
import { Where } from '@/core/common/types'
import { buildWhereClause } from '@/core/search/parser/buildWhereClause'
import { processCriteria } from '@/core/search/parser/processCriteria'
import { ParseContext } from '@/core/search/parser/types'
import { TSearchQueryBuilderOptions } from '@/core/search/search.types'

import { parseLevel } from './parseLevel'

export const parse = (
  input: Where,
  options: TSearchQueryBuilderOptions = { nodeAlias: DEFAULT_RECORD_ALIAS }
) => {
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
