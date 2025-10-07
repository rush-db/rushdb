import { isArray } from '@/common/utils/isArray'
import { isObject } from '@/common/utils/isObject'
import { toBoolean } from '@/common/utils/toBolean'
import {
  RUSHDB_KEY_ID,
  ROOT_RECORD_ALIAS,
  RUSHDB_KEY_ID_ALIAS,
  RUSHDB_VALUE_EMPTY_ARRAY,
  RUSHDB_KEY_PROPERTIES_META
} from '@/core/common/constants'
import {
  AggregateFn,
  Aggregate,
  AggregateCollectFn,
  AggregateCollectNestedFn,
  AliasesMap,
  AggregateCountFn,
  AggregateTimeBucketFn
} from '@/core/common/types'
import { PROPERTY_WILDCARD_PROJECTION } from '@/core/search/parser/constants'
import { AggregateContext } from '@/core/search/parser/types'
import { isNestedAggregate, safeGdsSimilarity, wrapInCurlyBraces } from '@/core/search/parser/utils'
import { SORT_ASC } from '@/core/search/search.constants'
import { TSearchSort } from '@/core/search/search.types'

import { buildOrderByClause, buildSortCriteria } from './orderBy'
import { pagination } from './pagination'
import { label } from './pickRecordLabel'

function apocSortMapsArray(arrayClause: string, orderBy?: TSearchSort) {
  const sortCriteria = buildSortCriteria(orderBy)

  const orderByKeyPart = Object.entries(sortCriteria).map(([property, direction]) => {
    return `"${direction.toLowerCase() === SORT_ASC ? '^' : ''}${property}"`
  })[0]

  return `apoc.coll.sortMaps(${arrayClause}, ${orderByKeyPart})`
}

function apocSortArray(arrayClause: string, orderBy?: TSearchSort) {
  return `apoc.coll.sort(apoc.coll.flatten(${arrayClause}))`
}

function apocUniqArray(arrayClause: string) {
  return `apoc.coll.toSet(${arrayClause})`
}

function apocRemoveFromArray(arrayClause: string) {
  return `apoc.coll.removeAll(${arrayClause}, ["${RUSHDB_VALUE_EMPTY_ARRAY}"])`
}

function parseAggregate(aggregate: Aggregate, aliasesMap: AliasesMap, ctx: AggregateContext) {
  // Process each aggregation instruction
  for (const [returnAlias, instruction] of Object.entries(aggregate)) {
    if (typeof instruction === 'string') {
      // Simple field selection
      const [recordAlias, ...fieldDescriptors] = instruction.split('.')
      const propertyNameRaw = fieldDescriptors.join('.')
      const recordQueryVariable = aliasesMap[recordAlias]

      const propertyName = propertyNameRaw === RUSHDB_KEY_ID_ALIAS ? RUSHDB_KEY_ID : propertyNameRaw

      ctx.fieldsInCollect.push(`\`${returnAlias}\`: ${recordQueryVariable}.\`${propertyName}\``)
    } else {
      const aggAlias = instruction.alias || '$record'
      if (aliasesMap[aggAlias]) {
        // Handle aggregation functions
        // Mutate a shallow copy to avoid side effects if we need the resolved alias downstream
        const resolvedInstruction = { ...instruction, alias: aggAlias }
        ctx.withAggregations.push(
          buildAggregationFunction(resolvedInstruction as AggregateFn, returnAlias, aliasesMap)
        )

        if ('aggregate' in instruction) {
          parseAggregate(instruction.aggregate, aliasesMap, {
            ...ctx,
            alias: aggAlias,
            returnAlias
          })
        } else {
          ctx.fieldsInCollect.push(`\`${returnAlias}\``)
        }

        if ('orderBy' in instruction) {
          let orderClause

          if (isObject(instruction.orderBy)) {
            orderClause = buildOrderByClause(
              Object.entries(instruction.orderBy).reduce((acc, [key, sortDirection]) => {
                acc[key] = sortDirection
                return acc
              }, {}),
              aliasesMap[aggAlias]
            )
          }

          if (typeof instruction.orderBy === 'string') {
            orderClause = buildOrderByClause(instruction.orderBy, aliasesMap[aggAlias])
          }

          ctx.orderClauses.push(orderClause)
        }
      }
    }
  }

  for (const groupAlias of ctx.groupBy) {
    if (!Object.keys(aggregate).includes(groupAlias)) {
      const [recordAlias, ...fieldDescriptors] = groupAlias.split('.')
      const propertyNameRaw = fieldDescriptors.join('.')
      const recordQueryVariable = aliasesMap[recordAlias]

      const propertyName = propertyNameRaw === RUSHDB_KEY_ID_ALIAS ? RUSHDB_KEY_ID : propertyNameRaw

      ctx.withAggregations.push(`${recordQueryVariable}.\`${propertyName}\` AS \`${propertyName}\``)
    }
  }
}

export function buildAggregation(aggregate: Aggregate, aliasesMap: AliasesMap, groupBy: string[]) {
  if (isObject(aggregate) && Object.keys(aggregate).length) {
    const isNested = isNestedAggregate(aggregate)

    if (isNested) {
      // Add first level aliases to RETURN clause
      const fieldsInCollect: string[] = [
        PROPERTY_WILDCARD_PROJECTION,
        `${label()}`,
        ...Object.keys(aggregate).map((key) => `\`${key}\``)
      ]

      const nestedAggregation = parseBottomUpQuery(
        aggregate as Record<string, AggregateCollectNestedFn>,
        ROOT_RECORD_ALIAS,
        aliasesMap
      )

      return {
        withPart: nestedAggregation.map((projection) => projection.withStatement).join('\n'),
        returnPart: `DISTINCT record {${fieldsInCollect.join(', ')}} AS records`
      }
    }

    const ctx: AggregateContext = {
      fieldsInCollect: toBoolean(groupBy) ? [] : [PROPERTY_WILDCARD_PROJECTION, `${label()}`],
      withAggregations: [],
      orderClauses: [],
      groupBy
    }

    parseAggregate(aggregate, aliasesMap, ctx)

    const groupByApplied = ctx.groupBy && toBoolean(ctx.groupBy) && isArray(ctx.groupBy)

    const withPart =
      ctx.withAggregations.length ?
        groupByApplied ? `WITH ${ctx.withAggregations.join(', ')}`
        : `WITH record, ${ctx.withAggregations.join(', ')}`
      : ''

    const returnPart =
      groupByApplied ?
        wrapInCurlyBraces(
          [...ctx.fieldsInCollect, ...ctx.groupBy]
            .map((variable, index) => {
              // We may want to get aggregated value as-is, for example:
              // {"labels":["HS_DEAL"],"aggregate":{"totalAmount":{"fn":"sum","field":"amount","alias":"$record"}},"groupBy":["totalAmount"]}
              // In this example we only care about the aggregated value (totalAmount) itself and do not
              // want to introduce an additional grouping dimension (e.g. a field like status or stage).
              // Listing the aggregation key inside groupBy tells the query planner to treat the
              // aggregation output as the grouping key so no extra key projection is added.
              // Pattern:
              //   aggregate: { totalAmount: { fn: 'sum', field: 'amount', alias: '$record' } }
              //   groupBy:  [ 'totalAmount' ]
              // This yields a single row (or one row per distinct aggregated value if multiple appear),
              // exposing the aggregated metric without forcing you to pick a real property key.
              if (Object.keys(aggregate).includes(variable)) {
                return undefined
              }

              if (ctx.groupBy.indexOf(variable) !== -1) {
                const [recordAlias, ...fieldDescriptors] = variable.split('.')
                const propertyNameRaw = fieldDescriptors.join('.')

                // @TODO: throw error if alias is missing
                // const recordQueryVariable = aliasesMap[recordAlias]

                const propertyName = propertyNameRaw === RUSHDB_KEY_ID_ALIAS ? RUSHDB_KEY_ID : propertyNameRaw

                return `\`${propertyName}\`:\`${propertyName}\``
              }

              return `${variable}:${variable}`
            })
            .filter(toBoolean)
            .join(', ')
        ) + ' as records'
      : `DISTINCT record {${ctx.fieldsInCollect.join(', ')}} as records`

    return {
      withPart,
      returnPart
    }
  }

  const fieldsInCollect: string[] = [PROPERTY_WILDCARD_PROJECTION, `${label()}`]

  // No aggregations provided
  return {
    withPart: '',
    returnPart: `DISTINCT record {${fieldsInCollect.join(', ')}} AS records`,
    refs: fieldsInCollect
  }
}

// Function to handle each aggregation function
export function buildAggregationFunction(
  instruction: AggregateFn,
  returnAlias: string,
  aliasesMap: AliasesMap
): string {
  const alias = instruction.alias || '$record'
  if (aliasesMap[alias]) {
    const recordAlias = aliasesMap[alias]
    const fieldAlias = `\`${instruction.field}\``
    const asPart = `\`${returnAlias}\``

    switch (instruction.fn) {
      case 'collect':
        return buildCollectFunction(instruction, returnAlias, aliasesMap)
      case 'count':
        return buildCountFunction(instruction, returnAlias, aliasesMap)
      case 'timeBucket':
        return buildTimeBucketFunction(instruction, returnAlias, aliasesMap)
      case 'sum':
        return `sum(${recordAlias}.${fieldAlias}) AS ${asPart}`
      case 'avg':
        if (
          typeof instruction.precision === 'number' &&
          instruction.precision >= 0 &&
          parseInt(instruction.precision.toString()) === instruction.precision
        ) {
          if (instruction.precision === 0) {
            return `toInteger(avg(${recordAlias}.${fieldAlias})) AS ${asPart}`
          }
          return `round(avg(${recordAlias}.${fieldAlias}), ${instruction.precision}) AS ${asPart}`
        }
        return `avg(${recordAlias}.${fieldAlias}) AS ${asPart}`
      case 'min':
        return `min(${recordAlias}.${fieldAlias}) AS ${asPart}`
      case 'max':
        return `max(${recordAlias}.${fieldAlias}) AS ${asPart}`

      // gds
      case 'gds.similarity.cosine':
      case 'gds.similarity.jaccard':
      case 'gds.similarity.euclidean':
      case 'gds.similarity.euclideanDistance':
      case 'gds.similarity.overlap':
      case 'gds.similarity.pearson':
        return safeGdsSimilarity(
          instruction.fn,
          recordAlias,
          instruction.field,
          instruction.query as unknown as string,
          asPart
        )

      default:
        throw new Error(`Unsupported aggregation function: ${JSON.stringify(instruction)}`)
    }
  }
}

// Handle collect function
export function buildCollectFunction(
  instruction: AggregateCollectFn,
  returnAlias: string,
  aliasesMap: AliasesMap
): string {
  // by default, unique = true
  const unique = instruction.unique === false ? '' : 'DISTINCT '

  const { skip, limit } = pagination(instruction.skip, instruction.limit)

  const hasFieldDescriptor = toBoolean(instruction.field)

  const propertyName = hasFieldDescriptor ? `.\`${instruction.field}\`` : ''

  const alias = aliasesMap[instruction.alias || '$record']

  if (hasFieldDescriptor) {
    const clause = apocRemoveFromArray(apocSortArray(`collect(${unique}${alias}${propertyName})`))

    const pagination = `[${skip}..${limit}]`
    const variableAssertion = ` AS \`${returnAlias}\``

    if (unique) {
      return apocUniqArray(clause) + pagination + variableAssertion
    }

    return clause + pagination + variableAssertion
  }

  return `${apocSortMapsArray(
    `collect(${unique}${alias}${propertyName} {${PROPERTY_WILDCARD_PROJECTION}, ${label(alias)}})`,
    instruction.orderBy
  )}[${skip}..${limit}] AS \`${returnAlias}\``
}

// Handle count function
export function buildCountFunction(
  instruction: AggregateCountFn,
  returnAlias: string,
  aliasesMap: AliasesMap
): string {
  const unique = instruction.unique === false ? '' : 'DISTINCT '

  const hasFieldDescriptor = toBoolean(instruction.field)

  const alias = aliasesMap[instruction.alias || '$record']

  const propertyName = hasFieldDescriptor ? `.${instruction.field}` : ''

  return `count(${unique}${alias}${propertyName}) AS \`${returnAlias}\``
}

// Handle timeBucket function
export function buildTimeBucketFunction(
  instruction: AggregateTimeBucketFn,
  returnAlias: string,
  aliasesMap: AliasesMap
): string {
  const fieldAlias = `\`${instruction.field}\``
  const recordAlias = aliasesMap[instruction.alias || '$record']

  const granularity = instruction.granularity
  if (granularity === 'months') {
    if (!instruction.size || instruction.size <= 0 || !Number.isInteger(instruction.size)) {
      throw new Error('timeBucket: size must be a positive integer when granularity = months')
    }
  }

  // We assume the field is stored either as a temporal type or ISO8601 string. We always convert to datetime
  // Neo4j: datetime() will parse ISO8601; if already temporal it is idempotent.
  const dtExpr = `datetime(${recordAlias}.${fieldAlias})`

  // Guard
  const datetimeMetaCheck = `apoc.convert.fromJsonMap(${recordAlias}.\`${RUSHDB_KEY_PROPERTIES_META}\`).\`${instruction.field}\` = "datetime"`

  let bucketStartExpr: string
  switch (granularity) {
    case 'day':
      bucketStartExpr = `datetime({year: ${dtExpr}.year, month: ${dtExpr}.month, day: ${dtExpr}.day})`
      break
    case 'week':
      // ISO week: derive Monday as start of the week
      bucketStartExpr = `datetime.truncate('week', ${dtExpr})`
      break
    case 'month':
      bucketStartExpr = `datetime({year: ${dtExpr}.year, month: ${dtExpr}.month, day: 1})`
      break
    case 'quarter':
      // Quarter start month = 1 + 3 * floor((month-1)/3)
      bucketStartExpr = `datetime({year: ${dtExpr}.year, month: 1 + 3 * toInteger(floor((${dtExpr}.month - 1)/3)), day: 1})`
      break
    case 'year':
      bucketStartExpr = `datetime({year: ${dtExpr}.year, month: 1, day: 1})`
      break
    case 'months': {
      const size = instruction.size as number
      // Compute bucket index starting from month 1. Example size=2: months 1-2 -> bucket 0, 3-4 -> bucket1
      // bucketStartMonth = 1 + size * floor((month-1)/size)
      bucketStartExpr = `datetime({year: ${dtExpr}.year, month: 1 + ${size} * toInteger(floor((${dtExpr}.month - 1)/${size})), day: 1})`
      break
    }
    default:
      throw new Error(`Unsupported timeBucket granularity: ${granularity}`)
  }

  // We return the bucket start value itself. The caller can groupBy this alias to roll up counts, sums, etc.
  // Format as ISO8601 string to have stable comparison if needed
  return `CASE WHEN ${datetimeMetaCheck} THEN ${bucketStartExpr} ELSE null END AS ${returnAlias}`
}

interface ParsedStatement {
  withStatement: string
  recordVariables: string[]
}

export function parseBottomUpQuery(
  queryAggregate: Record<string, AggregateCollectNestedFn>,
  baseRecordName = ROOT_RECORD_ALIAS,
  aliasesMap: AliasesMap
): ParsedStatement[] {
  const statements: ParsedStatement[] = []

  function buildCollectStatement(
    key: string,
    config: AggregateCollectNestedFn,
    currentRecord: string
  ): string {
    const collectParts = []

    // If there are nested aggregates, include their aliases
    if (config.aggregate) {
      const nestedKeys = Object.keys(config.aggregate)
      collectParts.splice(1, 0, ...nestedKeys)
    }

    const collectPart = `collect(DISTINCT ${currentRecord} {${[
      PROPERTY_WILDCARD_PROJECTION,
      ...collectParts.map((variable) => `\`${variable}\``),
      label(currentRecord)
    ].join(', ')}})`

    const { skip, limit } = pagination(config.skip, config.limit)

    return `${apocSortMapsArray(collectPart, config.orderBy)}[${skip}..${limit}] AS \`${key}\``
  }

  function processLevel(
    level: Record<string, AggregateCollectNestedFn>,
    currentRecord: string,
    parentRecords: string[]
  ): void {
    const nestedStatements: string[] = []
    const currentLevelRecords: string[] = [...parentRecords]

    for (const [key, instruction] of Object.entries(level)) {
      let recordQueryVariable

      if (instruction.alias) {
        recordQueryVariable = aliasesMap[instruction.alias]
      } else if (typeof instruction === 'string') {
        const [recordAlias, ...fieldDescriptors] = (instruction as string).split('.')
        recordQueryVariable = aliasesMap[recordAlias]
      }

      if (instruction.aggregate) {
        // Process nested levels first (bottom-up)
        processLevel(instruction.aggregate, aliasesMap[instruction.alias], [
          ...currentLevelRecords,
          recordQueryVariable
        ])
      }

      const collectStatement = buildCollectStatement(key, instruction, recordQueryVariable)
      nestedStatements.push(collectStatement)
    }

    if (nestedStatements.length > 0) {
      // Combine all collect statements at this level
      const withPart = `WITH ${currentLevelRecords.join(', ')}`
      const collectPart = nestedStatements.join(', ')
      const withStatement = `${withPart}, ${collectPart}`

      statements.push({
        withStatement,
        recordVariables: [...currentLevelRecords]
      })

      // Update parentRecords to include only the necessary records for the next level
      parentRecords.length = parentRecords.length - 1
      parentRecords.push(currentRecord)
    }
  }

  processLevel(queryAggregate, baseRecordName, [baseRecordName])

  return statements
}
