import { isArray } from '@/common/utils/isArray'
import { isObject } from '@/common/utils/isObject'
import { toBoolean } from '@/common/utils/toBolean'
import { ROOT_RECORD_ALIAS, RUSHDB_LABEL_RECORD, RUSHDB_RELATION_VALUE } from '@/core/common/constants'
import { Relation, TraversalHops, Where } from '@/core/common/types'
import { parseLevel } from '@/core/search/parser/buildQuery'
import { TraversalQueryError } from '@/core/search/parser/errors'
import { ParseContext } from '@/core/search/parser/types'
import { traversalRelsVar } from '@/core/search/parser/utils'
import { resolveMaxTraversalHops } from '@/core/search/search.constants'
import { TSearchQueryBuilderOptions } from '@/core/search/search.types'

type NormalizedHops = { min: number; max: number | null } | { exact: number }

const normalizeHops = (
  relation: Relation | undefined,
  isCycle: boolean,
  maxHops: number
): NormalizedHops | null => {
  const hops: TraversalHops | undefined = isObject(relation) ? (relation as any).hops : undefined
  const floor = isCycle ? 2 : 1

  if (hops === undefined) {
    if (isCycle) {
      throw new TraversalQueryError(
        `'$cycle' requires '$relation' with 'hops', e.g. { "$relation": { "hops": { "min": 2, "max": 6 } } }.`
      )
    }
    return null
  }

  const assertBounded = (max: number | undefined): number | null => {
    if (max === undefined) {
      if (maxHops !== Infinity) {
        throw new TraversalQueryError(
          `'hops.max' is required: unbounded traversal is not allowed on this connection (max allowed hops: ${maxHops}).`
        )
      }
      return null
    }
    if (!Number.isInteger(max) || max < floor || max > maxHops) {
      throw new TraversalQueryError(
        `'hops.max' must be an integer between ${floor} and ${maxHops === Infinity ? 'unbounded' : maxHops}, got ${JSON.stringify(max)}.`
      )
    }
    return max
  }

  if (typeof hops === 'number') {
    if (!Number.isInteger(hops) || hops < floor || hops > maxHops) {
      throw new TraversalQueryError(
        `'hops' must be an integer between ${floor} and ${maxHops === Infinity ? 'unbounded' : maxHops}, got ${JSON.stringify(hops)}.`
      )
    }
    return { exact: hops }
  }

  if (isObject(hops)) {
    const max = assertBounded((hops as any).max)
    const min = (hops as any).min ?? floor

    if (!Number.isInteger(min) || min < floor || (max !== null && min > max)) {
      throw new TraversalQueryError(
        `'hops.min' must be an integer between ${floor} and 'hops.max', got ${JSON.stringify(min)}.`
      )
    }
    return { min, max }
  }

  throw new TraversalQueryError(
    `'hops' must be a number or { min?, max? } object, got ${JSON.stringify(hops)}.`
  )
}

const renderHops = (hops: NormalizedHops | null): string => {
  if (hops === null) {
    return ''
  }
  if ('exact' in hops) {
    return `*${hops.exact}`
  }
  return `*${hops.min}..${hops.max ?? ''}`
}

// The __RUSHDB__RELATION__VALUE__ edges connect records to property meta-nodes; an
// untyped variable-length pattern would happily hop record → property ← record and
// fabricate connectivity that doesn't exist in the user's data model. Intermediate
// nodes stay otherwise unconstrained — safe only under the invariant that user
// relationships connect same-project Record nodes.
const valueEdgeFilter = (relVar: string) => `all(r IN ${relVar} WHERE type(r) <> '${RUSHDB_RELATION_VALUE}')`

const buildRelationPart = (
  relation: Relation | undefined,
  relVar: string,
  isCycle: boolean,
  maxHops: number
): { pattern: string; needsValueEdgeFilter: boolean } => {
  // @FYI: ORDER HERE IS CRUCIAL
  // DO NOT CHANGE!

  if (isCycle && !isObject(relation)) {
    throw new TraversalQueryError(
      `'$cycle' requires '$relation' with 'hops', e.g. { "$relation": { "hops": { "min": 2, "max": 6 } } }.`
    )
  }

  if (!toBoolean(relation)) {
    return { pattern: '--', needsValueEdgeFilter: false }
  }

  if (typeof relation === 'string') {
    return { pattern: `-[:${relation}]-`, needsValueEdgeFilter: false }
  }

  if (!isObject(relation)) {
    return { pattern: '--', needsValueEdgeFilter: false }
  }

  const hops = normalizeHops(relation, isCycle, maxHops)
  // Variable-length patterns bind the relationship list when the untyped filter or
  // the cycle existence check (`relsN IS NOT NULL`) needs to reference it.
  const bindVar = hops !== null && (isCycle || !relation.type)
  const needsValueEdgeFilter = hops !== null && !relation.type

  const inner = `${bindVar ? relVar : ''}${relation.type ? `:${relation.type}` : ''}${renderHops(hops)}`
  let relationClause = inner ? `[${inner}]` : ''

  if ('direction' in relation) {
    if (relation.direction === 'in') {
      relationClause = relationClause ? `<-${relationClause}-` : '<--'
    } else if (relation.direction === 'out') {
      relationClause = relationClause ? `-${relationClause}->` : '-->'
    } else if (hops !== null) {
      // A silent undirected fallback is the most expensive shape a variable-length
      // pattern can take — reject instead.
      throw new TraversalQueryError(
        `'direction' must be "in" or "out" when 'hops' is set, got ${JSON.stringify(relation.direction)}.`
      )
    } else {
      // unrecognised direction — fall back to direction-agnostic traversal
      relationClause = `-${relationClause}-`
    }
  } else {
    relationClause = `-${relationClause}-`
  }

  return { pattern: relationClause, needsValueEdgeFilter }
}

export const parseSubQuery = (
  key: string,
  input: Where,
  options?: TSearchQueryBuilderOptions,
  ctx?: ParseContext
) => {
  const { $relation, $alias, $cycle, ...other } = input as any

  ctx.level += 1
  // Capture the level before parseLevel recursion mutates ctx.level.
  const level = ctx.level
  const maxHops = options?.maxHops ?? resolveMaxTraversalHops()

  if (toBoolean($cycle)) {
    if ($alias) {
      throw new TraversalQueryError(`'$cycle' does not accept '$alias': a cycle has no separate endpoint.`)
    }
    if (Object.keys(other).length) {
      throw new TraversalQueryError(
        `'$cycle' accepts only '$relation': criteria and nested labels have no endpoint to anchor to, got ${JSON.stringify(Object.keys(other))}.`
      )
    }

    const relVar = traversalRelsVar(level)
    const { pattern, needsValueEdgeFilter } = buildRelationPart($relation, relVar, true, maxHops)
    const filter = needsValueEdgeFilter ? valueEdgeFilter(relVar) : ''

    // The rel var takes the place of a node alias: the existence check in the final
    // `WITH ... WHERE` is `relsN IS NOT NULL`, and WITH is a projection barrier.
    ctx.nodeAliases.push(relVar)

    // Keyed by the level's record slot purely for sortQueryParts ordering; both
    // pattern endpoints are the parent alias — the block's label key is ignored.
    ctx.result[ROOT_RECORD_ALIAS + level] =
      `OPTIONAL MATCH (${options.nodeAlias})${pattern}(${options.nodeAlias})${
        filter ? ` WHERE ${filter}` : ''
      }`
    return
  }

  const nodeAlias = ROOT_RECORD_ALIAS + level
  ctx.nodeAliases.push(nodeAlias)

  if ($alias) {
    ctx.aliasesMap[$alias] = nodeAlias
  }

  const subQueryWhere = parseLevel(
    key,
    other,
    {
      ...options,
      maxHops,
      nodeAlias
    },
    ctx
  )

  const condition =
    isArray(subQueryWhere) ?
      subQueryWhere.filter(toBoolean).join(options.joinOperator ? ` ${options.joinOperator} ` : ' AND ')
    : subQueryWhere

  const relVar = traversalRelsVar(level)
  const { pattern, needsValueEdgeFilter } = buildRelationPart($relation, relVar, false, maxHops)

  const whereParts = [needsValueEdgeFilter ? valueEdgeFilter(relVar) : '', condition].filter(toBoolean)

  // @FYI: OPTIONAL MATCH is crucial for handling logical grouping between Nodes
  // where: { $or: { CAR: {...}, BIKE: {...} } }
  // Strict MATCH will cause empty result unless target record relates to both (car and bike)
  ctx.result[nodeAlias] =
    `OPTIONAL MATCH (${options.nodeAlias})${pattern}(${nodeAlias}:${RUSHDB_LABEL_RECORD}:\`${key}\`)${
      whereParts.length ? ` WHERE ${whereParts.join(' AND ')}` : ''
    }`
}
