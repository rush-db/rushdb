import { BadRequestException } from '@nestjs/common'

import { isArray } from '@/common/utils/isArray'
import { toBoolean } from '@/common/utils/toBolean'
import {
  RUSHDB_KEY_ID,
  RUSHDB_KEY_ID_ALIAS,
  RUSHDB_LABEL_RECORD,
  ROOT_RECORD_ALIAS
} from '@/core/common/constants'
import { AliasesMap, CollectExpr, Expr, SelectExprMap, TimeBucketExpr } from '@/core/common/types'
import { parseWhereClause } from '@/core/search/parser/buildQuery'
import { PROPERTY_WILDCARD_PROJECTION } from '@/core/search/parser/constants'
import { pagination } from '@/core/search/parser/pagination'
import { label } from '@/core/search/parser/pickRecordLabel'
import { apocSortMapsArray } from '@/core/search/parser/utils'

// ── Field ref resolution ─────────────────────────────────────────────────────

/**
 * Resolves a string expression like "$record.name" or "$employee" into a
 * Cypher variable reference using the aliasesMap.
 *
 * - "$record.name"  → record.`name`
 * - "$employee"     → record2   (the node variable itself, no field)
 * - "*"             → handled by $count caller (not resolved here)
 */
function resolveRef(ref: string, aliasesMap: AliasesMap): string {
  const dotIndex = ref.indexOf('.')

  if (dotIndex === -1) {
    // Alias-only reference: "$employee" → aliasesMap["$employee"]
    const variable = aliasesMap[ref]
    if (!variable) {
      throw new BadRequestException(
        `Unknown alias "${ref}" in select expression. ` +
          `Declare it via $alias in the where clause or use "$record" for the root record.`
      )
    }
    return variable
  }

  // Field reference: "$record.name" → aliasesMap["$record"].`name`
  const aliasKey = ref.substring(0, dotIndex)
  const fieldRaw = ref.substring(dotIndex + 1)
  const variable = aliasesMap[aliasKey]

  if (!variable) {
    throw new BadRequestException(
      `Unknown alias "${aliasKey}" in select expression "${ref}". ` +
        `Declare it via $alias in the where clause or use "$record" for the root record.`
    )
  }

  const fieldName = fieldRaw === RUSHDB_KEY_ID_ALIAS ? RUSHDB_KEY_ID : fieldRaw
  return `${variable}.\`${fieldName}\``
}

/**
 * Returns true when the expression is a simple field ref string (e.g. "$record.name").
 * These go directly into the RETURN projection rather than the WITH clause.
 */
export function isSimpleFieldRef(expr: Expr): expr is string {
  return typeof expr === 'string' && expr.startsWith('$')
}

// ── Core expression compiler ─────────────────────────────────────────────────

/**
 * Compile a single `Expr` node into a Cypher fragment string.
 *
 * `$collect` and `$timeBucket` must be handled by `compileSelectMap` because
 * they produce full AS-clauses, not inner fragments.
 */
export function compileExpr(expr: Expr, aliasesMap: AliasesMap, selectMap: SelectExprMap): string {
  // Primitives
  if (typeof expr === 'number') {
    return String(expr)
  }
  if (typeof expr === 'boolean') {
    return String(expr)
  }

  if (typeof expr === 'string') {
    return resolveRef(expr, aliasesMap)
  }

  if (!expr || typeof expr !== 'object') {
    throw new BadRequestException(`Invalid select expression: ${JSON.stringify(expr)}`)
  }

  // $ref — cross-expression alias reference
  if ('$ref' in expr) {
    const refKey = (expr as { $ref: string }).$ref
    if (!(refKey in selectMap)) {
      throw new BadRequestException(
        `$ref "${refKey}" does not reference a key defined in the same select expression map.`
      )
    }
    return `\`${refKey}\``
  }

  // $sum
  if ('$sum' in expr) {
    return `sum(${compileExpr((expr as any).$sum, aliasesMap, selectMap)})`
  }

  // $avg — supports optional $precision
  if ('$avg' in expr) {
    const inner = compileExpr((expr as any).$avg, aliasesMap, selectMap)
    const precision = (expr as any).$precision
    if (typeof precision === 'number' && Number.isInteger(precision) && precision >= 0) {
      if (precision === 0) {
        return `toInteger(avg(${inner}))`
      }
      return `round(avg(${inner}), ${precision})`
    }
    return `avg(${inner})`
  }

  // $count
  if ('$count' in expr) {
    const operand = (expr as any).$count
    if (operand === '*') {
      const recordVar = aliasesMap['$record'] ?? 'record'
      return `count(DISTINCT ${recordVar})`
    }
    return `count(DISTINCT ${compileExpr(operand, aliasesMap, selectMap)})`
  }

  // $min / $max
  if ('$min' in expr) {
    return `min(${compileExpr((expr as any).$min, aliasesMap, selectMap)})`
  }
  if ('$max' in expr) {
    return `max(${compileExpr((expr as any).$max, aliasesMap, selectMap)})`
  }

  // Math operators
  if ('$divide' in expr) {
    const [a, b] = (expr as any).$divide as [Expr, Expr]
    return `(${compileExpr(a, aliasesMap, selectMap)} / ${compileExpr(b, aliasesMap, selectMap)})`
  }
  if ('$multiply' in expr) {
    const [a, b] = (expr as any).$multiply as [Expr, Expr]
    return `(${compileExpr(a, aliasesMap, selectMap)} * ${compileExpr(b, aliasesMap, selectMap)})`
  }
  if ('$add' in expr) {
    const [a, b] = (expr as any).$add as [Expr, Expr]
    return `(${compileExpr(a, aliasesMap, selectMap)} + ${compileExpr(b, aliasesMap, selectMap)})`
  }
  if ('$subtract' in expr) {
    const [a, b] = (expr as any).$subtract as [Expr, Expr]
    return `(${compileExpr(a, aliasesMap, selectMap)} - ${compileExpr(b, aliasesMap, selectMap)})`
  }

  // $collect and $timeBucket must be compiled at the map level
  if ('$collect' in expr || '$timeBucket' in expr) {
    throw new Error(
      'Internal: $collect and $timeBucket must be handled by compileSelectMap, not compileExpr.'
    )
  }

  throw new BadRequestException(`Unknown select expression operator: ${JSON.stringify(expr)}`)
}

// ── from-based collect compiler ─────────────────────────────────────────────

function compileCollect(collectExpr: CollectExpr, returnAlias: string, aliasesMap: AliasesMap): string {
  if (!collectExpr.from) {
    throw new BadRequestException(
      `$collect requires either "from" (pre-declared alias) or "label" (inline traversal).`
    )
  }

  const unique = collectExpr.unique === false ? '' : 'DISTINCT '
  const { skip, limit } = pagination(collectExpr.skip, collectExpr.limit)
  const alias = aliasesMap[collectExpr.from]

  if (!alias) {
    throw new BadRequestException(
      `Unknown alias "${collectExpr.from}" in $collect expression. ` +
        `Declare it via $alias in the where clause.`
    )
  }

  const hasSelect = collectExpr.select && Object.keys(collectExpr.select).length > 0

  if (hasSelect) {
    // Build custom projection: { key: alias.field, ... }
    const projection = Object.entries(collectExpr.select!)
      .map(([key, fieldExpr]) => {
        if (typeof fieldExpr !== 'string' || !fieldExpr.startsWith('$')) {
          throw new BadRequestException(
            `select inside $collect only supports field references (e.g. "$employee.name"), ` +
              `got: ${JSON.stringify(fieldExpr)}`
          )
        }
        const compiled = resolveRef(fieldExpr, aliasesMap)
        return `${key}: ${compiled}`
      })
      .join(', ')

    const collectPart = `collect(${unique}${alias} {${projection}, ${label(alias)}})`
    return `${apocSortMapsArray(collectPart, collectExpr.orderBy)}[${skip}..${limit}] AS \`${returnAlias}\``
  }

  // No select → collect full records
  const collectPart = `collect(${unique}${alias} {${PROPERTY_WILDCARD_PROJECTION}, ${label(alias)}})`
  return `${apocSortMapsArray(collectPart, collectExpr.orderBy)}[${skip}..${limit}] AS \`${returnAlias}\``
}

// ── label-based nested collect compiler ──────────────────────────────────────

type NestedCollectResult = {
  /** OPTIONAL MATCH clauses in top-down order (parent before children). */
  matchClauses: string[]
  /** Complete WITH statements in bottom-up order (innermost first). */
  withStatements: string[]
}

/**
 * Recursively compiles a `$collect` expression that uses `label` (self-describing
 * inline traversal) into a chain of OPTIONAL MATCH + WITH collect statements.
 *
 * Variable names are allocated as `sel0`, `sel1`, … to avoid collisions with
 * the `record`, `record1`, `record2`, … variables produced by the `where` parser.
 *
 * `$self` in field references inside `$collect.select` resolves to the current
 * level's Cypher variable and is re-bound at every recursion depth.
 */
function compileNestedCollect(
  collectExpr: CollectExpr,
  parentVar: string,
  ancestorVars: string[],
  returnAlias: string,
  parentAliasesMap: AliasesMap,
  counter: { n: number }
): NestedCollectResult {
  const selfVar = `sel${counter.n++}`
  // $self resolves to the current level's node variable
  const localAliasesMap: AliasesMap = { ...parentAliasesMap, $self: selfVar }
  // selfPath = ancestors + this node; used as WITH prefix for one level deeper
  const selfPath = [...ancestorVars, selfVar]

  const matchClauses: string[] = []
  const withStatements: string[] = []
  // Tracks which select keys are nested $collect.label entries (already resolved by inner WITHs)
  const nestedCollectKeys: string[] = []

  // Depth-first: process nested $collect.label entries before building this level
  if (collectExpr.select) {
    for (const [key, expr] of Object.entries(collectExpr.select)) {
      if (expr && typeof expr === 'object' && '$collect' in (expr as object)) {
        const nestedCollect = (expr as any).$collect as CollectExpr
        if (nestedCollect.label) {
          const child = compileNestedCollect(nestedCollect, selfVar, selfPath, key, localAliasesMap, counter)
          // Child OPTIONAL MATCHes are appended; this level's match is unshifted below
          matchClauses.push(...child.matchClauses)
          // Child WITHs come before this level's WITH (innermost first)
          withStatements.push(...child.withStatements)
          nestedCollectKeys.push(key)
        }
      }
    }
  }

  // Build this level's OPTIONAL MATCH
  const labelPart = `${RUSHDB_LABEL_RECORD}:\`${collectExpr.label}\``
  let matchClause = `OPTIONAL MATCH (${parentVar})--(${selfVar}:${labelPart})`

  if (collectExpr.where) {
    const parsed = parseWhereClause(collectExpr.where as any, { nodeAlias: selfVar })
    const condition = parsed.queryParts[selfVar]
    if (condition) {
      matchClause += ` WHERE (${condition})`
    }
  }

  // This level's MATCH comes before child matches (top-down)
  matchClauses.unshift(matchClause)

  // Build collect projection
  const { skip, limit } = pagination(collectExpr.skip, collectExpr.limit)
  const unique = collectExpr.unique === false ? '' : 'DISTINCT '
  let collectPart: string

  if (collectExpr.select && Object.keys(collectExpr.select).length > 0) {
    const projectionParts: string[] = []
    for (const [key, expr] of Object.entries(collectExpr.select)) {
      if (nestedCollectKeys.includes(key)) {
        // Already computed by inner WITHs — reference by backtick alias
        projectionParts.push(`\`${key}\``)
      } else if (typeof expr === 'string' && expr.startsWith('$')) {
        projectionParts.push(`${key}: ${resolveRef(expr, localAliasesMap)}`)
      }
      // Aggregation expressions inside $collect.select: deferred (incompatible cardinality)
    }
    projectionParts.push(label(selfVar))
    collectPart = `collect(${unique}${selfVar} {${projectionParts.join(', ')}})`
  } else {
    // No select → collect full records
    collectPart = `collect(${unique}${selfVar} {${PROPERTY_WILDCARD_PROJECTION}, ${label(selfVar)}})`
  }

  // Build this level's WITH statement.
  // WITH prefix = ancestor vars (selfVar is consumed by collect and not carried forward).
  const withPrefix = ancestorVars.join(', ')
  const withStatement = `WITH ${withPrefix}, ${apocSortMapsArray(collectPart, collectExpr.orderBy)}[${skip}..${limit}] AS \`${returnAlias}\``

  // This WITH comes AFTER all child WITHs (bottom-up ordering)
  withStatements.push(withStatement)

  return { matchClauses, withStatements }
}

// ── TimeBucket compiler ───────────────────────────────────────────────────────

function compileTimeBucket(tbExpr: TimeBucketExpr, returnAlias: string, aliasesMap: AliasesMap): string {
  const fieldRef = resolveRef(tbExpr.field, aliasesMap)
  const { unit, size } = tbExpr

  // Validate size requirement for plural units
  const pluralUnits = ['months', 'hours', 'minutes', 'seconds', 'years']
  if (pluralUnits.includes(unit)) {
    if (!size || size <= 0 || !Number.isInteger(size)) {
      throw new BadRequestException(
        `$timeBucket: "size" must be a positive integer when "unit" is one of: ${pluralUnits.join(', ')}`
      )
    }
  }

  const datetimeMetaCheck = `any(t IN ['datetime', 'date'] WHERE ${fieldRef} IS NOT NULL)`

  let bucketStartExpr: string
  switch (unit) {
    case 'day':
      bucketStartExpr = `datetime({year: ${fieldRef}.year, month: ${fieldRef}.month, day: ${fieldRef}.day}).epochMillis`
      break
    case 'week':
      bucketStartExpr = `datetime({year: ${fieldRef}.year, week: ${fieldRef}.week}).epochMillis`
      break
    case 'month':
      bucketStartExpr = `datetime({year: ${fieldRef}.year, month: ${fieldRef}.month}).epochMillis`
      break
    case 'quarter':
      bucketStartExpr = `datetime({year: ${fieldRef}.year, month: (toInteger((${fieldRef}.month - 1) / 3) * 3) + 1}).epochMillis`
      break
    case 'year':
    case 'years':
      if (unit === 'years') {
        bucketStartExpr = `datetime({year: (toInteger(${fieldRef}.year / ${size}) * ${size})}).epochMillis`
      } else {
        bucketStartExpr = `datetime({year: ${fieldRef}.year}).epochMillis`
      }
      break
    case 'hour':
      bucketStartExpr = `datetime({year: ${fieldRef}.year, month: ${fieldRef}.month, day: ${fieldRef}.day, hour: ${fieldRef}.hour}).epochMillis`
      break
    case 'hours':
      bucketStartExpr = `datetime({year: ${fieldRef}.year, month: ${fieldRef}.month, day: ${fieldRef}.day, hour: (toInteger(${fieldRef}.hour / ${size}) * ${size})}).epochMillis`
      break
    case 'minute':
      bucketStartExpr = `datetime({year: ${fieldRef}.year, month: ${fieldRef}.month, day: ${fieldRef}.day, hour: ${fieldRef}.hour, minute: ${fieldRef}.minute}).epochMillis`
      break
    case 'minutes':
      bucketStartExpr = `datetime({year: ${fieldRef}.year, month: ${fieldRef}.month, day: ${fieldRef}.day, hour: ${fieldRef}.hour, minute: (toInteger(${fieldRef}.minute / ${size}) * ${size})}).epochMillis`
      break
    case 'second':
      bucketStartExpr = `datetime({year: ${fieldRef}.year, month: ${fieldRef}.month, day: ${fieldRef}.day, hour: ${fieldRef}.hour, minute: ${fieldRef}.minute, second: ${fieldRef}.second}).epochMillis`
      break
    case 'seconds':
      bucketStartExpr = `datetime({year: ${fieldRef}.year, month: ${fieldRef}.month, day: ${fieldRef}.day, hour: ${fieldRef}.hour, minute: ${fieldRef}.minute, second: (toInteger(${fieldRef}.second / ${size}) * ${size})}).epochMillis`
      break
    case 'months':
      bucketStartExpr = `datetime({year: ${fieldRef}.year, month: (toInteger((${fieldRef}.month - 1) / ${size}) * ${size}) + 1}).epochMillis`
      break
    default:
      throw new BadRequestException(`$timeBucket: unsupported unit "${unit}"`)
  }

  return `CASE WHEN ${datetimeMetaCheck} THEN ${bucketStartExpr} ELSE null END AS \`${returnAlias}\``
}

// ── $ref dependency analysis ──────────────────────────────────────────────────

function collectRefDependencies(expr: Expr): Set<string> {
  const deps = new Set<string>()

  function walk(e: Expr): void {
    if (e === null || typeof e !== 'object') {
      return
    }
    if (isArray(e)) {
      ;(e as unknown as Expr[]).forEach(walk)
      return
    }
    if ('$ref' in (e as object)) {
      deps.add((e as { $ref: string }).$ref)
      return
    }
    // Recurse into all object values
    for (const value of Object.values(e as object)) {
      if (value !== null && value !== undefined) {
        if (typeof value === 'object') {
          walk(value as Expr)
        }
      }
    }
  }

  walk(expr)
  return deps
}

/**
 * Topological sort of selectMap keys by $ref dependencies.
 * Returns an ordered array of keys (dependencies before dependents).
 * Throws if there is a circular $ref.
 */
function topoSort(selectMap: SelectExprMap): string[] {
  const keys = Object.keys(selectMap)
  const deps = new Map<string, Set<string>>()

  for (const [key, expr] of Object.entries(selectMap)) {
    deps.set(key, collectRefDependencies(expr))
  }

  const sorted: string[] = []
  const resolved = new Set<string>()
  const remaining = new Set<string>(keys)

  while (remaining.size > 0) {
    const sizeAtStart = remaining.size

    for (const key of [...remaining]) {
      const keyDeps = deps.get(key)!
      // Only keys that appear in the selectMap are real deps (not external refs)
      const internalDeps = [...keyDeps].filter((d) => keys.includes(d))
      if (internalDeps.every((d) => resolved.has(d))) {
        sorted.push(key)
        resolved.add(key)
        remaining.delete(key)
      }
    }

    if (remaining.size === sizeAtStart) {
      // No progress — circular dependency
      throw new BadRequestException(
        `Circular $ref detected in select expression. ` + `Remaining keys: ${[...remaining].join(', ')}`
      )
    }
  }

  return sorted
}

// ── Top-level select map compiler ────────────────────────────────────────────

/**
 * Compiles a `SelectExprMap` into Cypher WITH + RETURN clause parts.
 *
 * Handles:
 * - Simple field refs (go into the RETURN projection directly)
 * - Aggregation + math expressions (go into WITH clause, referenced in RETURN)
 * - $collect and $timeBucket (go into WITH clause)
 * - $ref with topological ordering (multiple WITH layers when needed)
 * - groupBy mode (no `record {}` wrapping; explicit field projection)
 */
export function compileSelectMap(
  selectMap: SelectExprMap,
  aliasesMap: AliasesMap,
  groupBy: string[] = []
): { withPart: string; returnPart: string; matchPart: string } {
  const groupByApplied = toBoolean(groupBy) && isArray(groupBy) && groupBy.length > 0

  // Topological order ensures $ref targets appear before their dependents
  const sortedKeys = topoSort(selectMap)

  // fieldsInCollect: fragments that go inside record { ... } in the RETURN clause
  const fieldsInCollect: string[] = groupByApplied ? [] : [PROPERTY_WILDCARD_PROJECTION, label()]

  // Each entry is an array of "expr AS `alias`" strings for one WITH statement
  // We need multiple WITHs when $ref spans multiple layers
  const withLayers: string[][] = []
  const resolvedInWith = new Set<string>() // keys whose alias is defined in a WITH

  // Accumulates OPTIONAL MATCH + WITH chains from label-based $collect entries
  const nestedMatchClauses: string[] = []
  const nestedWithStatements: string[] = []
  const nestedCounter = { n: 0 }

  for (const key of sortedKeys) {
    const expr = selectMap[key]

    if (isSimpleFieldRef(expr)) {
      // Simple field reference — goes in RETURN projection, no WITH needed
      const compiled = resolveRef(expr as string, aliasesMap)
      fieldsInCollect.push(`\`${key}\`: ${compiled}`)
      continue
    }

    // label-based $collect — self-describing inline traversal
    if (expr && typeof expr === 'object' && '$collect' in (expr as object)) {
      const collectExpr = (expr as any).$collect as CollectExpr
      if (collectExpr.label) {
        const result = compileNestedCollect(
          collectExpr,
          ROOT_RECORD_ALIAS,
          [ROOT_RECORD_ALIAS],
          key,
          aliasesMap,
          nestedCounter
        )
        nestedMatchClauses.push(...result.matchClauses)
        nestedWithStatements.push(...result.withStatements)
        if (!groupByApplied) {
          fieldsInCollect.push(`\`${key}\``)
        }
        resolvedInWith.add(key)
        continue
      }
    }

    // Determine which WITH layer this expression belongs to
    // (must go after all its $ref dependencies)
    const refDeps = [...collectRefDependencies(expr)].filter((d) => Object.keys(selectMap).includes(d))
    let targetLayer = 0
    for (const dep of refDeps) {
      for (let i = withLayers.length - 1; i >= 0; i--) {
        if (withLayers[i].some((clause) => clause.endsWith(`\`${dep}\``))) {
          targetLayer = Math.max(targetLayer, i + 1)
          break
        }
      }
    }

    // Compile expression
    let compiled: string
    if ('$collect' in (expr as object)) {
      compiled = compileCollect((expr as any).$collect as CollectExpr, key, aliasesMap)
    } else if ('$timeBucket' in (expr as object)) {
      compiled = compileTimeBucket((expr as any).$timeBucket as TimeBucketExpr, key, aliasesMap)
    } else {
      const fragment = compileExpr(expr, aliasesMap, selectMap)
      compiled = `${fragment} AS \`${key}\``
    }

    // Ensure the layer array exists
    while (withLayers.length <= targetLayer) {
      withLayers.push([])
    }
    withLayers[targetLayer].push(compiled)
    resolvedInWith.add(key)

    // Track in fieldsInCollect (reference to the WITH-level alias)
    fieldsInCollect.push(`\`${key}\``)
  }

  // In groupBy mode: inject dimension fields (non-selectMap groupBy keys) into the first
  // WITH layer so they are in scope for ORDER BY and RETURN.
  if (groupByApplied) {
    for (const groupAlias of groupBy) {
      if (Object.keys(selectMap).includes(groupAlias)) {
        continue // Self-groupBy — the aggregation result is already in withLayers
      }
      const dotIndex = groupAlias.indexOf('.')
      if (dotIndex === -1) {
        continue
      }
      const aliasKey = groupAlias.substring(0, dotIndex)
      const fieldRaw = groupAlias.substring(dotIndex + 1)
      const variable = aliasesMap[aliasKey]
      if (!variable) {
        throw new BadRequestException(
          `groupBy variable "${groupAlias}" references unknown alias "${aliasKey}". ` +
            `Ensure "${aliasKey}" is declared in the select or where clause.`
        )
      }
      const fieldName = fieldRaw === RUSHDB_KEY_ID_ALIAS ? RUSHDB_KEY_ID : fieldRaw
      if (withLayers.length === 0) {
        withLayers.push([])
      }
      withLayers[0].push(`${variable}.\`${fieldName}\` AS \`${fieldName}\``)
    }
  }

  // Build WITH statements
  // Each WITH layer carries forward `record` and all aliases from prior layers
  const withStatements: string[] = []
  const priorAliases: string[] = []

  for (const layer of withLayers) {
    if (layer.length === 0) {
      continue
    }

    let withPrefix: string
    if (groupByApplied) {
      withPrefix = priorAliases.length > 0 ? `${priorAliases.join(', ')}, ` : ''
    } else {
      withPrefix = priorAliases.length > 0 ? `record, ${priorAliases.join(', ')}, ` : 'record, '
    }

    withStatements.push(`WITH ${withPrefix}${layer.join(', ')}`)

    // Collect the alias names produced by this layer (extract from "... AS `key`")
    for (const clause of layer) {
      const match = clause.match(/AS `([^`]+)`$/)
      if (match) {
        priorAliases.push(`\`${match[1]}\``)
      }
    }
  }

  const withPart = [...withStatements, ...nestedWithStatements].filter(Boolean).join('\n')
  const matchPart = nestedMatchClauses.join('\n')

  // Build RETURN part
  let returnPart: string

  if (groupByApplied) {
    // Build groupBy projection: { fieldAlias: fieldAlias, ... }
    const returnEntries: string[] = []

    // Include all select keys (both field refs and aggregations)
    for (const key of sortedKeys) {
      if (groupBy.includes(key)) {
        continue
      } // groupBy self-aggregation keys are handled below
      if (resolvedInWith.has(key)) {
        returnEntries.push(`\`${key}\`:\`${key}\``)
      }
      // Field refs in groupBy context: not yet handled here (below)
    }

    // Add groupBy dimension keys (e.g. "$record.status")
    for (const groupAlias of groupBy) {
      if (Object.keys(selectMap).includes(groupAlias)) {
        // Self-groupBy on an aggregated key — include the aggregation result in RETURN
        returnEntries.push(`\`${groupAlias}\`:\`${groupAlias}\``)
        continue
      }
      const [aliasKey, ...fieldParts] = groupAlias.split('.')
      const fieldRaw = fieldParts.join('.')

      if (!aliasesMap[aliasKey]) {
        throw new BadRequestException(
          `groupBy variable "${groupAlias}" references unknown alias "${aliasKey}". ` +
            `Ensure "${aliasKey}" is declared in the select or where clause.`
        )
      }

      const fieldName = fieldRaw === RUSHDB_KEY_ID_ALIAS ? RUSHDB_KEY_ID : fieldRaw
      returnEntries.push(`\`${fieldName}\`:\`${fieldName}\``)
    }

    returnPart = `{${returnEntries.join(', ')}} as records`
  } else {
    returnPart = `DISTINCT record {${fieldsInCollect.join(', ')}} as records`
  }

  return { withPart, returnPart, matchPart }
}
