import { isArray } from '@/common/utils/isArray'
import { isObject } from '@/common/utils/isObject'
import { toBoolean } from '@/common/utils/toBolean'
import { DEFAULT_RECORD_ALIAS } from '@/core/common/constants'
import { Relation, Where } from '@/core/common/types'
import { parseLevel } from '@/core/search/parser/buildQuery'
import { ParseContext } from '@/core/search/parser/types'
import { TSearchQueryBuilderOptions } from '@/core/search/search.types'

const buildRelationPart = (relation?: Relation) => {
  // @FYI: ORDER HERE IS CRUCIAL
  // DO NOT CHANGE!

  if (!toBoolean(relation)) {
    return '--'
  }

  if (typeof relation === 'string') {
    return `-[:${relation}]-`
  }

  if (!isObject(relation)) {
    return '--'
  }

  let relationClause = relation.type ? `[:${relation.type}]` : ''

  if ('direction' in relation) {
    if (relation.direction === 'in') {
      relationClause = relationClause ? `<-${relationClause}-` : '<--'
    } else {
      relationClause = relationClause ? `-${relationClause}->` : '-->'
    }
  } else {
    relationClause = `-${relationClause}-`
  }

  return relationClause
}

export const parseSubQuery = (
  key: string,
  input: Where,
  options?: TSearchQueryBuilderOptions,
  ctx?: ParseContext
) => {
  const { $relation, $alias, ...other } = input as any

  ctx.level += 1
  const nodeAlias = DEFAULT_RECORD_ALIAS + ctx.level
  ctx.nodeAliases.push(nodeAlias)

  if ($alias) {
    ctx.aliasesMap[$alias] = nodeAlias
  }

  const subQueryWhere = parseLevel(
    key,
    other,
    {
      ...options,
      nodeAlias
    },
    ctx
  )

  const condition =
    isArray(subQueryWhere) ?
      subQueryWhere.filter(toBoolean).join(options.joinOperator ? ` ${options.joinOperator} ` : ' AND ')
    : subQueryWhere

  const rel = buildRelationPart($relation)

  // @FYI: OPTIONAL MATCH is crucial for handling logical grouping between Nodes
  // where: { $or: { CAR: {...}, BIKE: {...} } }
  // Strict MATCH will cause empty result unless target record relates to both (car and bike)
  ctx.result[nodeAlias] = `OPTIONAL MATCH (${options.nodeAlias})${rel}(${nodeAlias}:${key})${
    toBoolean(condition) ? ` WHERE ${condition}` : ''
  }`
}
