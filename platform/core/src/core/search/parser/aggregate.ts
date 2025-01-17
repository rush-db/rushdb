import { isObject } from '@/common/utils/isObject'
import { toBoolean } from '@/common/utils/toBolean'
import {
  RUSHDB_KEY_ID,
  RUSHDB_KEY_PROPERTIES_META,
  DEFAULT_RECORD_ALIAS,
  RUSHDB_KEY_ID_ALIAS,
  RUSHDB_VALUE_EMPTY_ARRAY
} from '@/core/common/constants'
import {
  AggregateFn,
  Aggregate,
  AggregateCollectFn,
  AggregateCollectNestedFn,
  AliasesMap
} from '@/core/common/types'
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

function parseAggregate(
  aggregate: Aggregate,
  aliasesMap: AliasesMap,
  ctx: {
    fieldsInCollect: string[]
    withAggregations: string[]
    orderClauses: string[]
    alias?: string
    returnAlias?: string
  }
) {
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
      if (aliasesMap[instruction.alias]) {
        // Handle aggregation functions

        ctx.withAggregations.push(buildAggregationFunction(instruction, returnAlias, aliasesMap))

        if ('aggregate' in instruction) {
          parseAggregate(instruction.aggregate, aliasesMap, {
            ...ctx,
            alias: instruction.alias,
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
              aliasesMap[instruction.alias]
            )
          }

          if (typeof instruction.orderBy === 'string') {
            orderClause = buildOrderByClause(instruction.orderBy, aliasesMap[instruction.alias])
          }

          ctx.orderClauses.push(orderClause)
        }
      }
    }
  }
}

export function buildAggregation(aggregate: Aggregate, aliasesMap: AliasesMap) {
  if (isObject(aggregate) && Object.keys(aggregate).length) {
    const isNested = Object.values(aggregate).some((instruction) => isObject(instruction.aggregate))

    if (isNested) {
      // Add first level aliases to RETURN clause
      const fieldsInCollect: string[] = [
        '.*',
        `${label()}`,
        ...Object.keys(aggregate).map((key) => `\`${key}\``)
      ]

      const nestedAggregation = parseBottomUpQuery(
        aggregate as Record<string, AggregateCollectNestedFn>,
        DEFAULT_RECORD_ALIAS,
        aliasesMap
      )

      return {
        withPart: nestedAggregation.map((projection) => projection.withStatement).join('\n'),
        recordPart: `collect(DISTINCT record {${fieldsInCollect.join(', ')}}) AS records`
      }
    } else {
      const fieldsInCollect: string[] = [
        `${RUSHDB_KEY_ID}: record.${RUSHDB_KEY_ID}`,
        `${RUSHDB_KEY_PROPERTIES_META}: record.${RUSHDB_KEY_PROPERTIES_META}`,
        `${label()}`
      ]

      const withAggregations = []
      const orderClauses = []

      parseAggregate(aggregate, aliasesMap, {
        fieldsInCollect,
        withAggregations,
        orderClauses
      })

      const withPart = withAggregations.length ? `WITH record, ${withAggregations.join(', ')}` : ''

      // Combine the return clause
      return {
        withPart,
        recordPart: `collect(DISTINCT record {${fieldsInCollect.join(', ')}}) AS records`
      }
    }
  }

  return {
    withPart: '',
    recordPart: `collect(DISTINCT record {.*, ${label()}}) AS records`
  }
}

// Function to handle each aggregation function
export function buildAggregationFunction(
  instruction: AggregateFn,
  returnAlias: string,
  aliasesMap: AliasesMap
): string {
  if (aliasesMap[instruction.alias]) {
    const recordAlias = aliasesMap[instruction.alias]
    const fieldAlias = `\`${instruction.field}\``
    const asPart = `\`${returnAlias}\``

    switch (instruction.fn) {
      case 'collect':
        return buildCollectFunction(instruction, returnAlias, aliasesMap)
      case 'count':
        return buildCountFunction(instruction, returnAlias, aliasesMap)
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
  // by default uniq = true
  const uniq = instruction.uniq === false ? '' : 'DISTINCT '

  const { skip, limit } = pagination(instruction.skip, instruction.limit)

  const hasFieldDescriptor = toBoolean(instruction.field)

  const propertyName = hasFieldDescriptor ? `.\`${instruction.field}\`` : ''

  const alias = aliasesMap[instruction.alias]

  if (hasFieldDescriptor) {
    const clause = apocRemoveFromArray(apocSortArray(`collect(${uniq}${alias}${propertyName})`))

    const pagination = `[${skip}..${limit}]`
    const variableAssertion = ` AS \`${returnAlias}\``

    if (uniq) {
      return apocUniqArray(clause) + pagination + variableAssertion
    }

    return clause + pagination + variableAssertion
  }

  return `${apocSortMapsArray(
    `collect(${uniq}${alias}${propertyName} {.*, ${label(alias)}})`,
    instruction.orderBy
  )}[${skip}..${limit}] AS \`${returnAlias}\``
}

// Handle count function
export function buildCountFunction(
  instruction: AggregateFn,
  returnAlias: string,
  aliasesMap: AliasesMap
): string {
  const uniq = 'uniq' in instruction && instruction.uniq ? 'DISTINCT ' : ''

  const hasFieldDescriptor = toBoolean(instruction.field)

  const alias = aliasesMap[instruction.alias]

  const propertyName = hasFieldDescriptor ? `.${instruction.field}` : ''

  return `count(${uniq}${alias}${propertyName}) AS \`${returnAlias}\``
}

interface ParsedStatement {
  withStatement: string
  recordVariables: string[]
}

export function parseBottomUpQuery(
  queryAggregate: Record<string, AggregateCollectNestedFn>,
  baseRecordName = DEFAULT_RECORD_ALIAS,
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
      '.*',
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
