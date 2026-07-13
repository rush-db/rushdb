import { BadRequestException, Injectable } from '@nestjs/common'

import { isArray } from '@/common/utils/isArray'
import { toBoolean } from '@/common/utils/toBolean'
import {
  ROOT_RECORD_ALIAS,
  RUSHDB_KEY_ID,
  RUSHDB_KEY_PROJECT_ID,
  RUSHDB_KEY_PROPERTIES_META,
  RUSHDB_LABEL_PROPERTY,
  RUSHDB_LABEL_RECORD,
  RUSHDB_RELATION_DEFAULT,
  RUSHDB_RELATION_VALUE
} from '@/core/common/constants'
import { MaybeArray, Where } from '@/core/common/types'
import { RELATION_DIRECTION_IN, RELATION_DIRECTION_OUT } from '@/core/entity/entity.constants'
import { TRelationDirection } from '@/core/entity/entity.types'
import { RelationshipSearchDto } from '@/core/relationships/dto/relationship-search.dto'
import { RESERVED_RELATIONSHIP_PROPERTY_KEYS } from '@/core/relationships/relationship-properties'
import { SearchDto } from '@/core/search/dto/search.dto'
import { buildAggregation } from '@/core/search/parser'
import { compileSelectMap, normalizeSelectExpr } from '@/core/search/parser'
import {
  buildLabelsClause,
  buildPagination,
  buildQuery,
  buildQueryClause,
  isOrderByAggregatedField,
  parseWhereClause,
  sort
} from '@/core/search/parser/buildQuery'
import { buildRelatedQueryPart } from '@/core/search/parser/buildRelatedRecordQueryPart'
import { PROPERTY_WILDCARD_PROJECTION } from '@/core/search/parser/constants'
import { projectIdInline } from '@/core/search/parser/projectIdInline'
import { singleLabelPart } from '@/core/search/parser/singleLabelPart'
import { QueryBuilder } from '@/database/QueryBuilder'

import { label } from '../search/parser/pickRecordLabel'

@Injectable()
export class EntityQueryService {
  upsertRecord({ rewrite = false }: { rewrite?: boolean } = {}) {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`CYPHER 25`)
      .append(`WITH $record as r, datetime() as time`)
      .append(
        `WITH *, apoc.map.fromPairs([property IN r.properties | [property.name, property.type]]) AS typesMap,`
      )
      .append(
        `apoc.map.fromPairs([property IN r.properties | [property.name, property.value]]) AS valuesMap,`
      )
      .append(`coalesce($mergeBy, []) as mergeBy`)
      .append(`WITH *, CASE WHEN size(mergeBy)=0 THEN keys(valuesMap) ELSE mergeBy END as keysToMatch`)
      .append(`OPTIONAL MATCH (record:${RUSHDB_LABEL_RECORD} { ${projectIdInline()} })`)
      .append(
        `WHERE (r.label IS NULL OR ANY(l IN labels(record) WHERE l = r.label)) AND ALL(k IN keysToMatch WHERE record[k] = valuesMap[k])`
      )
      .append(`CALL (*) {`)
      .append(`  WHEN record IS NULL THEN {`)
      .append(
        `    CREATE (newRecord:${RUSHDB_LABEL_RECORD} { ${RUSHDB_KEY_ID}: r.id, ${projectIdInline()} })`
      )
      .append(`    RETURN newRecord AS activeRecord`)
      .append(`  } ELSE {`)
      .append(`    RETURN record AS activeRecord`)
      .append(`  }`)
      .append(`}`)
      .append(`WITH r, time, activeRecord AS record`)

    queryBuilder.append(
      `CALL apoc.create.addLabels(record, ["${RUSHDB_LABEL_RECORD}", coalesce(r.label, "${RUSHDB_LABEL_RECORD}")]) YIELD node as upsertLabelResult`
    )

    if (rewrite) {
      queryBuilder
        .append(
          `OPTIONAL MATCH (record)-[oldValue:${RUSHDB_RELATION_VALUE}]-(oldProperty:${RUSHDB_LABEL_PROPERTY})`
        )
        .append(`DELETE oldValue`)
        .append(this.processProps())
    } else {
      queryBuilder.append(this.processPropsAppend())
    }

    queryBuilder.append(`RETURN record {${PROPERTY_WILDCARD_PROJECTION}, ${label()}} as data`)

    return queryBuilder.getQuery()
  }

  createRecord() {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`WITH $record as r, datetime() as time`)
      .append(`CREATE (record:${RUSHDB_LABEL_RECORD} {${RUSHDB_KEY_ID}: r.id, ${projectIdInline()}})`)
      .append(`WITH *`)
      .append(
        `CALL apoc.create.addLabels(record, ["${RUSHDB_LABEL_RECORD}", coalesce(r.label, "${RUSHDB_LABEL_RECORD}")]) YIELD node as labelCreationResult`
      )
      .append(this.processProps())
      .append(`RETURN record {${PROPERTY_WILDCARD_PROJECTION}, ${label()}} as data`)

    return queryBuilder.getQuery()
  }

  getEntity() {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`MATCH (record:${RUSHDB_LABEL_RECORD} { ${RUSHDB_KEY_ID}: $id, ${projectIdInline()} })`)
      .append(`RETURN record {${PROPERTY_WILDCARD_PROJECTION}, ${label()}} as data`)

    return queryBuilder.getQuery()
  }

  editRecord() {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`WITH $record as r, datetime() as time`)
      .append(`MATCH (record:${RUSHDB_LABEL_RECORD} { ${RUSHDB_KEY_ID}: r.id, ${projectIdInline()} })`)
      .append(`WITH *`)
      .append(
        `OPTIONAL MATCH (record)-[oldValue:${RUSHDB_RELATION_VALUE}]-(oldProperty:${RUSHDB_LABEL_PROPERTY})`
      )
      .append(`DELETE oldValue`)
      .append(this.processProps())

    return queryBuilder.getQuery()
  }

  deleteRecord(config?: { cascadeDeleteRecords?: boolean }) {
    const queryBuilder = new QueryBuilder()

    queryBuilder.append(
      `MATCH (record:${RUSHDB_LABEL_RECORD} { ${RUSHDB_KEY_ID}: $id, ${projectIdInline()} })`
    )

    if (config?.cascadeDeleteRecords) {
      queryBuilder
        .append(`OPTIONAL MATCH (record)-[*]->(cascadeRecord:${RUSHDB_LABEL_RECORD})`)
        .append(`DETACH DELETE cascadeRecord`)
    }

    queryBuilder.append(`DETACH DELETE record`)

    return queryBuilder.getQuery()
  }

  importRecords(withResults = false) {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`WITH $records as recordsToCreate, datetime() as time`)
      .append(`UNWIND recordsToCreate as r`)
      .append(`CREATE (record:${RUSHDB_LABEL_RECORD} { ${RUSHDB_KEY_ID}: r.id, ${projectIdInline()} })`)
      .append(this.processProps())

    if (withResults) {
      queryBuilder.append(
        `RETURN collect({draftId: r.id, persistedId: record.${RUSHDB_KEY_ID}}) as idmap, collect(DISTINCT record {${PROPERTY_WILDCARD_PROJECTION}, ${label()}}) as data`
      )
    } else {
      queryBuilder.append(`RETURN collect({draftId: r.id, persistedId: record.${RUSHDB_KEY_ID}}) as idmap`)
    }

    return queryBuilder.getQuery()
  }

  /**
   * Batch upsert of Records produced by import.
   * Global mergeBy semantics:
   *  - If $mergeBy is empty OR null -> use all property keys for each record.
   *  - Otherwise restrict match to provided list.
   * mergeStrategy:
   *  - 'rewrite' removes existing value relationships before applying new ones.
   *  - 'append' (default) appends without deleting existing ones.
   * Parameters expected:
   *  - records: [{ id, label, properties: [{name, value, type, id, created}] }, ...]
   *  - projectId
   *  - mergeBy (array<string>|null)
   *  - rewrite (boolean)
   *  - returnResult (boolean)
   */
  importUpsertRecords({
    withResults = false,
    rewrite = false,
    mergeBy,
    label: recordLabel
  }: { withResults?: boolean; rewrite?: boolean; mergeBy?: string[]; label?: string } = {}) {
    const queryBuilder = new QueryBuilder()

    // When mergeBy is provided, inline the keys as static property accesses
    // (record.`key` = valuesMap.`key`). Dynamic access (record[k]) defeats the planner:
    // it forces a per-UNWIND-row scan of every record in the project — O(batch × graph) —
    // which is what blew the transaction budget on large projects. Static accesses let
    // Neo4j use a property index when one exists. Keys are inlined into the query text,
    // so escape backticks to keep user-supplied names from breaking out of the identifier.
    const staticMergeKeys = (mergeBy ?? []).filter((k) => typeof k === 'string' && k.trim().length > 0)
    const useStaticMergeKeys = staticMergeKeys.length > 0

    // Callers group an upsert batch by business label before calling this method, so every
    // row in $records shares `label` — inline it as a static pattern label (record:Record:`Label`)
    // instead of matching every :Record in the project and filtering by r.label afterward. This
    // scopes the match to one business label's node set rather than the whole project. Rows with
    // no label (rare: an unlabeled root import) are called without `label` and fall back to the
    // original untyped match + runtime label check.
    const labelPart = recordLabel ? `:\`${recordLabel.replace(/`/g, '``')}\`` : ''
    const labelGuard = labelPart ? '' : '(r.label IS NULL OR ANY(l IN labels(record) WHERE l = r.label)) AND '

    queryBuilder
      .append(`WITH $records as recordsToUpsert, datetime() as time, coalesce($mergeBy, []) as mergeBy`)
      .append(`UNWIND recordsToUpsert as r`)
      .append(
        `WITH *, apoc.map.fromPairs([property IN r.properties | [property.name, property.type]]) AS typesMap,`
      )
      .append(`apoc.map.fromPairs([property IN r.properties | [property.name, property.value]]) AS valuesMap`)

    if (useStaticMergeKeys) {
      const mergePredicates = staticMergeKeys
        .map((key) => {
          const escaped = `\`${key.replace(/`/g, '``')}\``
          return `record.${escaped} = valuesMap.${escaped}`
        })
        .join(' AND ')

      queryBuilder
        .append(`OPTIONAL MATCH (record:${RUSHDB_LABEL_RECORD}${labelPart} { ${projectIdInline()} })`)
        .append(`WHERE ${labelGuard}${mergePredicates}`)
    } else {
      queryBuilder
        .append(`WITH *, CASE WHEN size(mergeBy)=0 THEN keys(valuesMap) ELSE mergeBy END as keysToMatch`)
        .append(`OPTIONAL MATCH (record:${RUSHDB_LABEL_RECORD}${labelPart} { ${projectIdInline()} })`)
        .append(`WHERE ${labelGuard}ALL(k IN keysToMatch WHERE record[k] = valuesMap[k])`)
    }

    queryBuilder
      .append(
        `WITH * CALL apoc.do.when(record IS NULL, 'CREATE (newRecord:${RUSHDB_LABEL_RECORD} { ${RUSHDB_KEY_ID}: r.id, ${projectIdInline()} }) RETURN newRecord', 'RETURN NULL', { r:r, projectId: $projectId }) YIELD value`
      )
      .append(`WITH *, coalesce(value.newRecord, record) AS record`)
      .append(
        `CALL apoc.create.addLabels(record, ["${RUSHDB_LABEL_RECORD}", coalesce(r.label, "${RUSHDB_LABEL_RECORD}")]) YIELD node as upsertLabelResult`
      )

    if (rewrite) {
      queryBuilder
        .append(
          `OPTIONAL MATCH (record)-[oldValue:${RUSHDB_RELATION_VALUE}]-(oldProperty:${RUSHDB_LABEL_PROPERTY})`
        )
        .append(`DELETE oldValue`)
        .append(this.processProps())
    } else {
      queryBuilder.append(this.processPropsAppend())
    }

    if (withResults) {
      queryBuilder.append(
        `RETURN collect({draftId: r.id, persistedId: record.${RUSHDB_KEY_ID}}) as idmap, collect(DISTINCT record {${PROPERTY_WILDCARD_PROJECTION}, ${label()}}) as data`
      )
    } else {
      queryBuilder.append(`RETURN collect({draftId: r.id, persistedId: record.${RUSHDB_KEY_ID}}) as idmap`)
    }

    return queryBuilder.getQuery()
  }

  findRecords({ id, searchQuery }: { searchQuery?: SearchDto; id?: string }) {
    const relatedQueryPart = buildRelatedQueryPart(id)
    const labelPart = singleLabelPart(searchQuery?.labels)

    const { queryClauses, sortedQueryParts, parsedWhere, aliasesMap } = buildQuery(searchQuery)

    const pagination = buildPagination(searchQuery)
    const orderByAggregatedField = isOrderByAggregatedField(searchQuery)
    const sortParams = sort(searchQuery.orderBy, orderByAggregatedField ? null : ROOT_RECORD_ALIAS)

    // Determine which output-shaping path to use:
    // - select (new expr-style) → normalizeSelectExpr returns a SelectExprMap → compileSelectMap
    // - aggregate (legacy fn-style) → normalizeSelectExpr returns null → buildAggregation
    // TODO: Remove aggregate branch when aggregate DSL is dropped
    const normalizedSelect = normalizeSelectExpr({
      select: searchQuery?.select,
      aggregate: searchQuery?.aggregate
    })

    const {
      withPart: aggregateProjections,
      returnPart,
      matchPart
    } =
      normalizedSelect ?
        compileSelectMap(normalizedSelect, aliasesMap, searchQuery?.groupBy ?? [])
      : { ...buildAggregation(searchQuery?.aggregate, aliasesMap, searchQuery?.groupBy ?? []), matchPart: '' }

    // convert a clause array to string
    const normalizedQueryClauses = queryClauses.filter(toBoolean).join(`\n`)

    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`MATCH ${relatedQueryPart}(record:${RUSHDB_LABEL_RECORD}${labelPart} { ${projectIdInline()} })`)
      .append(normalizedQueryClauses)

    if (this.needsWhereBarrier(parsedWhere)) {
      const wherePart = parsedWhere.where ? `WHERE ${parsedWhere.where}` : ''

      queryBuilder.append(`WITH ${parsedWhere.nodeAliases.join(', ')} ${wherePart}`.trim())
    }

    // @FYI: Pagination/sort must be applied AFTER the traversal WITH...WHERE barrier
    // above — otherwise SKIP/LIMIT truncates the root record scan before related-label
    // matches are even evaluated, silently dropping rows that only appear once the
    // traversal filter is applied. For the aggregated-order-by case, ORDER BY must
    // additionally follow aggregateProjections since it sorts by the computed field.
    if (!orderByAggregatedField) {
      queryBuilder.append(`${sortParams} ${pagination}`.trim())
    }

    if (matchPart) {
      queryBuilder.append(matchPart)
    }

    queryBuilder.append(aggregateProjections)

    if (orderByAggregatedField) {
      queryBuilder.append(`${sortParams} ${pagination}`.trim())
    }

    queryBuilder.append(`RETURN ${returnPart}`)

    return queryBuilder.getQuery()
  }

  /**
   * The WITH…WHERE barrier is needed when the where tree references traversal aliases
   * (nodeAliases beyond the root) or carries alias-free predicates — a $cycle-only
   * query compiles to an EXISTS subquery with nodeAliases === ['record'] but a
   * non-empty where, which the alias-count check alone would silently drop.
   */
  private needsWhereBarrier(parsedWhere: { nodeAliases: string[]; where: string }): boolean {
    return parsedWhere.nodeAliases.filter(toBoolean).length > 1 || toBoolean(parsedWhere.where)
  }

  getRecordsCount({ id, searchQuery }: { id?: string; searchQuery?: SearchDto }) {
    const relatedQueryPart = buildRelatedQueryPart(id)
    const labelPart = singleLabelPart(searchQuery?.labels)

    const { sortedQueryParts, parsedWhere } = buildQuery(searchQuery)

    const queryClauses = buildQueryClause({
      queryParts: sortedQueryParts,
      labelClause: buildLabelsClause(searchQuery?.labels)
    })

    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`MATCH ${relatedQueryPart}(record:${RUSHDB_LABEL_RECORD}${labelPart} { ${projectIdInline()} })`)
      .append(queryClauses.join(`\n`))

    if (this.needsWhereBarrier(parsedWhere)) {
      const wherePart = parsedWhere.where ? `WHERE ${parsedWhere.where}` : ''

      queryBuilder.append(`WITH ${parsedWhere.nodeAliases.join(', ')} ${wherePart}`.trim())
    }

    queryBuilder.append('RETURN count(DISTINCT record) as total')

    return queryBuilder.getQuery()
  }

  getEntityPropertiesKeys({ id, searchQuery }: { id?: string; searchQuery?: SearchDto }) {
    const labelPart = singleLabelPart(searchQuery?.labels)

    const queryBuilder = new QueryBuilder()

    if (toBoolean(id)) {
      queryBuilder.append(
        `MATCH (record:${RUSHDB_LABEL_RECORD}${labelPart} { ${projectIdInline()}, ${RUSHDB_KEY_ID}: "${id}" })`
      )
    } else {
      const { sortedQueryParts, parsedWhere } = buildQuery(searchQuery)

      const queryClauses = buildQueryClause({
        queryParts: sortedQueryParts,
        labelClause: buildLabelsClause(searchQuery?.labels)
      })

      queryBuilder
        .append(`MATCH (record:${RUSHDB_LABEL_RECORD}${labelPart} { ${projectIdInline()} })`)
        .append(queryClauses.join(`\n`))

      if (this.needsWhereBarrier(parsedWhere)) {
        const wherePart = parsedWhere.where ? `WHERE ${parsedWhere.where}` : ''

        queryBuilder.append(`WITH ${parsedWhere.nodeAliases.join(', ')} ${wherePart}`.trim())
      }
    }

    queryBuilder
      .append(`UNWIND keys(record) as propertyName`)
      .append(`RETURN collect(DISTINCT propertyName) as fields`)

    return queryBuilder.getQuery()
  }

  getEntityLabels(searchQuery?: SearchDto) {
    const { sortedQueryParts, parsedWhere } = buildQuery({
      // Omitting 'labels', 'orderBy', 'skip', 'limit', 'aggregate'
      where: searchQuery?.where
    })

    const queryClauses = buildQueryClause({
      queryParts: sortedQueryParts,
      labelClause: buildLabelsClause(searchQuery?.labels)
    })

    const wherePart = parsedWhere.where ? `WHERE ${parsedWhere.where}` : ''

    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`MATCH (record:${RUSHDB_LABEL_RECORD} { ${projectIdInline()} })`)
      .append(queryClauses.join(`\n`))
      .append(`WITH ${parsedWhere.nodeAliases.join(', ')} ${wherePart}`.trim())
      .append(
        `WITH DISTINCT record, [label IN labels(record) WHERE label <> "${RUSHDB_LABEL_RECORD}"] as recordLabels`
      )
      .append(`RETURN count(recordLabels) as total, recordLabels[0] as label`)

    return queryBuilder.getQuery()
  }

  linkRecords() {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`WITH $relations as relations`)
      .append(`UNWIND relations as relation`)
      .append(
        `MATCH (source:${RUSHDB_LABEL_RECORD} { ${RUSHDB_KEY_ID}: relation.source, ${projectIdInline()} }), (target:${RUSHDB_LABEL_RECORD} { ${RUSHDB_KEY_ID}: relation.target, ${projectIdInline()} })`
      )
      .append(
        `CALL apoc.create.relationship(source, coalesce(relation.type, '${RUSHDB_RELATION_DEFAULT}'), coalesce(relation.properties, {}), target) YIELD rel`
      )
      .append(`RETURN rel`)

    return queryBuilder.getQuery()
  }

  private quoteIdentifier(value: string) {
    return `\`${String(value).replace(/`/g, '')}\``
  }

  private quoteString(value: string) {
    return `"${String(value).replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`
  }

  private relationshipLiteral(value: unknown): string {
    if (typeof value === 'string') {
      return this.quoteString(value)
    }
    if (value === null) {
      return 'null'
    }
    if (Array.isArray(value)) {
      return `[${value.map((item) => this.relationshipLiteral(item)).join(', ')}]`
    }
    return String(value)
  }

  private relationshipValues(field: string) {
    return `coalesce(apoc.convert.toList(rel.${this.quoteIdentifier(field)}), [])`
  }

  private relationshipDatetimeValue(value: unknown) {
    if (typeof value === 'string') {
      return `datetime(${this.quoteString(value)})`
    }
    return undefined
  }

  private relationshipTypePredicate(input: any): string {
    const relType = 'type(rel)'
    if (typeof input === 'string') {
      return `${relType} = ${this.quoteString(input)}`
    }
    if (input && typeof input === 'object' && !Array.isArray(input)) {
      return Object.entries(input)
        .map(([operator, value]) => {
          switch (operator) {
            case '$eq':
              return `${relType} = ${this.relationshipLiteral(value)}`
            case '$ne':
              return `${relType} <> ${this.relationshipLiteral(value)}`
            case '$in':
              return Array.isArray(value) ? `${relType} IN ${this.relationshipLiteral(value)}` : ''
            case '$nin':
              return Array.isArray(value) ? `NOT (${relType} IN ${this.relationshipLiteral(value)})` : ''
            case '$contains':
              return typeof value === 'string' ? `${relType} =~ ${this.quoteString(`(?i).*${value}.*`)}` : ''
            case '$startsWith':
              return typeof value === 'string' ? `${relType} STARTS WITH ${this.quoteString(value)}` : ''
            case '$endsWith':
              return typeof value === 'string' ? `${relType} ENDS WITH ${this.quoteString(value)}` : ''
            case '$exists':
              return (
                typeof value === 'boolean' ?
                  value ? `${relType} IS NOT NULL`
                  : `${relType} IS NULL`
                : ''
              )
            default:
              return ''
          }
        })
        .filter(toBoolean)
        .join(' AND ')
    }
    return ''
  }

  private relationshipTypeCheck(field: string, value: unknown) {
    const values = this.relationshipValues(field)
    if (typeof value !== 'string') {
      return ''
    }
    const typeMap: Record<string, string[]> = {
      string: ['STRING'],
      number: ['INTEGER', 'FLOAT'],
      boolean: ['BOOLEAN'],
      datetime: ['STRING'],
      null: ['NULL'],
      vector: ['LIST OF FLOAT', 'LIST OF INTEGER']
    }
    const expectedTypes = typeMap[value] ?? [value.toUpperCase()]
    const typeList = `[${expectedTypes.map((type) => this.quoteString(type)).join(', ')}]`
    const base = `any(value IN ${values} WHERE apoc.meta.cypher.type(value) IN ${typeList})`

    if (value === 'datetime') {
      return `any(value IN ${values} WHERE apoc.meta.cypher.type(value) = "STRING" AND value =~ ${this.quoteString(
        '^\\\\d{4}-\\\\d{2}-\\\\d{2}T.*'
      )})`
    }

    return base
  }

  private relationshipPropertyPredicate(field: string, input: any): string {
    const valueExpr = `rel.${this.quoteIdentifier(field)}`
    const values = this.relationshipValues(field)

    if (input === null) {
      return `${valueExpr} IS NULL`
    }

    if (Array.isArray(input)) {
      return `${valueExpr} = ${this.relationshipLiteral(input)}`
    }

    if (input === undefined) {
      return ''
    }

    if (typeof input !== 'object') {
      return `any(value IN ${values} WHERE value = ${this.relationshipLiteral(input)})`
    }

    return Object.entries(input)
      .map(([operator, value]) => {
        switch (operator) {
          case '$type':
            return this.relationshipTypeCheck(field, value)
          case '$eq':
            return value === null ?
                `${valueExpr} IS NULL`
              : `any(value IN ${values} WHERE value = ${this.relationshipLiteral(value)})`
          case '$ne':
            return value === null ?
                `${valueExpr} IS NOT NULL`
              : `any(value IN ${values} WHERE value <> ${this.relationshipLiteral(value)})`
          case '$gt':
          case '$gte':
          case '$lt':
          case '$lte': {
            const opMap = { $gt: '>', $gte: '>=', $lt: '<', $lte: '<=' } as Record<string, string>
            if (typeof value === 'number') {
              return `any(value IN ${values} WHERE value ${opMap[operator]} ${value})`
            }
            const datetimeValue = this.relationshipDatetimeValue(value)
            return datetimeValue ?
                `any(value IN ${values} WHERE datetime(value) ${opMap[operator]} ${datetimeValue})`
              : ''
          }
          case '$contains':
            return typeof value === 'string' ?
                `any(value IN ${values} WHERE toString(value) =~ ${this.quoteString(`(?i).*${value}.*`)})`
              : ''
          case '$startsWith':
            return typeof value === 'string' ?
                `any(value IN ${values} WHERE toString(value) STARTS WITH ${this.quoteString(value)})`
              : ''
          case '$endsWith':
            return typeof value === 'string' ?
                `any(value IN ${values} WHERE toString(value) ENDS WITH ${this.quoteString(value)})`
              : ''
          case '$in':
            return Array.isArray(value) ?
                `any(value IN ${values} WHERE value IN ${this.relationshipLiteral(value)})`
              : ''
          case '$nin':
            return Array.isArray(value) ?
                `none(value IN ${values} WHERE value IN ${this.relationshipLiteral(value)})`
              : ''
          case '$exists':
            return (
              typeof value === 'boolean' ?
                value ? `${valueExpr} IS NOT NULL`
                : `${valueExpr} IS NULL`
              : ''
            )
          default:
            return ''
        }
      })
      .filter(toBoolean)
      .join(' AND ')
  }

  private relationshipWherePredicate(where: Where | undefined): {
    predicate: string
    direction?: TRelationDirection
  } {
    if (!where || Object.keys(where).length === 0) {
      return { predicate: '' }
    }

    const build = (input: any): string => {
      if (!input || typeof input !== 'object' || Array.isArray(input)) {
        return ''
      }

      return Object.entries(input)
        .map(([key, value]) => {
          switch (key) {
            case 'direction':
              return ''
            case 'type':
              return this.relationshipTypePredicate(value)
            case '$and':
            case '$or':
            case '$xor': {
              const operator = key.slice(1).toUpperCase()
              const items = Array.isArray(value) ? value : [value]
              return items
                .map((item) => build(item))
                .filter(toBoolean)
                .map((item) => `(${item})`)
                .join(` ${operator} `)
            }
            case '$nor': {
              const items = Array.isArray(value) ? value : [value]
              const condition = items
                .map((item) => build(item))
                .filter(toBoolean)
                .map((item) => `(${item})`)
                .join(' OR ')
              return condition ? `NOT (${condition})` : ''
            }
            case '$not': {
              const condition = build(value)
              return condition ? `NOT (${condition})` : ''
            }
            default:
              return this.relationshipPropertyPredicate(key, value)
          }
        })
        .filter(toBoolean)
        .map((part) => `(${part})`)
        .join(' AND ')
    }

    const direction = (where as any).direction
    return {
      predicate: build(where),
      direction:
        direction === RELATION_DIRECTION_IN || direction === RELATION_DIRECTION_OUT ? direction : undefined
    }
  }

  private relationshipSourceEndpoint({
    id,
    source
  }: {
    id?: string
    source?: RelationshipSearchDto['source']
  }): RelationshipSearchDto['source'] {
    if (!id) {
      return source
    }

    const idWhere = { $id: id }
    const where =
      source?.where && Object.keys(source.where).length > 0 ? { $and: [source.where, idWhere] } : idWhere

    return { ...(source ?? {}), where }
  }

  private relationshipPattern({
    direction,
    includeBothDirections
  }: {
    direction?: TRelationDirection
    includeBothDirections?: boolean
  }) {
    if (direction === RELATION_DIRECTION_IN) {
      return '<-[rel]-'
    }
    if (includeBothDirections) {
      return '-[rel]-'
    }
    return '-[rel]->'
  }

  private endpointPredicates(
    endpoint: RelationshipSearchDto['source'],
    alias: string
  ): { matches: string[]; predicates: string[] } {
    const predicates: string[] = []
    const matches: string[] = []

    if (endpoint?.labels?.length) {
      const labels = endpoint.labels.filter(toBoolean).map((item) => this.quoteString(item))
      if (labels.length) {
        predicates.push(`any(label IN labels(${alias}) WHERE label IN [${labels.join(', ')}])`)
      }
    }

    if (endpoint?.where && Object.keys(endpoint.where).length > 0) {
      const parsed = parseWhereClause(endpoint.where, { nodeAlias: alias })
      const sorted = Object.keys(parsed.queryParts)
        .sort((a, b) => a.localeCompare(b))
        .map((key) => parsed.queryParts[key])

      for (const clause of sorted) {
        if (!clause) {
          continue
        }
        if (/^\s*OPTIONAL MATCH\b/i.test(clause)) {
          matches.push(clause)
        } else {
          predicates.push(clause.replace(/^\s*WHERE\s+/i, ''))
        }
      }
    }

    return { matches, predicates }
  }

  private relationshipOrderBy(orderBy: RelationshipSearchDto['orderBy']) {
    if (!orderBy || typeof orderBy !== 'object') {
      return ''
    }

    const clauses = Object.entries(orderBy)
      .filter(([, direction]) => direction === 'asc' || direction === 'desc')
      .map(([field, direction]) =>
        field === 'type' ? `type(rel) ${direction}` : `rel.${this.quoteIdentifier(field)} ${direction}`
      )

    return clauses.length ? `ORDER BY ${clauses.join(', ')}` : ''
  }

  private relationshipProjection() {
    const reservedKeys = `[${RESERVED_RELATIONSHIP_PROPERTY_KEYS.map((key) => this.quoteString(key)).join(', ')}]`
    return `
CASE
  WHEN startNode(rel) = source THEN {
    sourceId: source.${RUSHDB_KEY_ID}, ${label('source', 'sourceLabel')},
    targetId: target.${RUSHDB_KEY_ID}, ${label('target', 'targetLabel')},
    type: type(rel),
    direction: '${RELATION_DIRECTION_OUT}',
    properties: apoc.map.removeKeys(properties(rel), ${reservedKeys})
  }
  ELSE {
    sourceId: target.${RUSHDB_KEY_ID}, ${label('target', 'sourceLabel')},
    targetId: source.${RUSHDB_KEY_ID}, ${label('source', 'targetLabel')},
    type: type(rel),
    direction: '${RELATION_DIRECTION_IN}',
    properties: apoc.map.removeKeys(properties(rel), ${reservedKeys})
  }
END`
  }

  getRecordRelations({
    id,
    searchQuery,
    pagination
  }: {
    id?: string
    searchQuery?: RelationshipSearchDto
    pagination?: Pick<SearchDto, 'skip' | 'limit'>
  }) {
    const sourceEndpoint = this.relationshipSourceEndpoint({ id, source: searchQuery?.source })
    const source = this.endpointPredicates(sourceEndpoint, 'source')
    const target = this.endpointPredicates(searchQuery?.target, 'target')
    const relationshipWhere = this.relationshipWherePredicate(searchQuery?.where)
    const relationPart = this.relationshipPattern({
      direction: relationshipWhere.direction,
      includeBothDirections: Boolean(id && !relationshipWhere.direction)
    })
    const predicates = [...source.predicates, ...target.predicates, relationshipWhere.predicate].filter(
      toBoolean
    )
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(
        `MATCH (source:${RUSHDB_LABEL_RECORD} { ${projectIdInline()} })${relationPart}(target:${RUSHDB_LABEL_RECORD} { ${projectIdInline()} })`
      )
      .append([...source.matches, ...target.matches].join('\n'))
      .append(predicates.length ? `WHERE ${predicates.join(' AND ')}` : '')
      .append(this.relationshipOrderBy(searchQuery?.orderBy))
      .append(buildPagination(pagination ?? searchQuery ?? {}))
      .append(`RETURN DISTINCT ${this.relationshipProjection()} AS relation`)

    return queryBuilder.getQuery()
  }

  getRecordRelationsCount({ id, searchQuery }: { id?: string; searchQuery?: RelationshipSearchDto }) {
    const sourceEndpoint = this.relationshipSourceEndpoint({ id, source: searchQuery?.source })
    const source = this.endpointPredicates(sourceEndpoint, 'source')
    const target = this.endpointPredicates(searchQuery?.target, 'target')
    const relationshipWhere = this.relationshipWherePredicate(searchQuery?.where)
    const relationPart = this.relationshipPattern({
      direction: relationshipWhere.direction,
      includeBothDirections: Boolean(id && !relationshipWhere.direction)
    })
    const predicates = [...source.predicates, ...target.predicates, relationshipWhere.predicate].filter(
      toBoolean
    )
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(
        `MATCH (source:${RUSHDB_LABEL_RECORD} { ${projectIdInline()} })${relationPart}(target:${RUSHDB_LABEL_RECORD} { ${projectIdInline()} })`
      )
      .append([...source.matches, ...target.matches].join('\n'))
      .append(predicates.length ? `WHERE ${predicates.join(' AND ')}` : '')
    queryBuilder.append(`RETURN count(DISTINCT rel) as total`)

    return queryBuilder.getQuery()
  }

  processProps() {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`WITH *,`)
      .append(`apoc.map.fromPairs([property IN r.properties | [property.name, property.type]]) AS typesMap,`)
      .append(
        `apoc.map.fromPairs([property IN r.properties | [property.name, property.value]]) AS valuesMap,`
      )
      .append(`[label IN labels(record) WHERE label <> "${RUSHDB_LABEL_RECORD}"] as recordLabels,`)
      .append(
        `[key IN keys(record) WHERE key <> "${RUSHDB_KEY_ID}" AND key <> "${RUSHDB_KEY_PROJECT_ID}"] as keysToCleanup`
      )
      .append(`CALL apoc.create.removeProperties(record, keysToCleanup) YIELD node as recordCleanup`)
      .append(`CALL apoc.create.removeLabels(record, [recordLabels[0]]) YIELD node as recordLabelRemove`)
      .append(
        `CALL apoc.create.addLabels(record, [coalesce(r.label, recordLabels[0])]) YIELD node as recordLabelUpdate`
      )
      .append(`SET record.${RUSHDB_KEY_PROPERTIES_META} = apoc.convert.toJson(typesMap)`)
      .append(`SET record += valuesMap`)
      .append(`WITH *`)
      .append(`UNWIND r.properties as prop`)
      .append(
        `MERGE (p:${RUSHDB_LABEL_PROPERTY} { name: prop.name, type: coalesce(prop.type, 'string'), projectId: $projectId, metadata: coalesce(prop.metadata, "")})`
      )
      .append(
        `ON CREATE SET p.created = coalesce(prop.created, time), p.id = prop.id, p.metadata = coalesce(prop.metadata, "")`
      )
      .append(`MERGE (p)-[rel:${RUSHDB_RELATION_VALUE}]->(record)`)

    return queryBuilder.getQuery()
  }

  processPropsAppend() {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`WITH *,`)
      .append(`apoc.map.fromPairs([property IN r.properties | [property.name, property.type]]) AS typesMap,`)
      .append(
        `apoc.map.fromPairs([property IN r.properties | [property.name, property.value]]) AS valuesMap,`
      )
      .append(`[label IN labels(record) WHERE label <> "${RUSHDB_LABEL_RECORD}"] as recordLabels`)
      .append(`WITH *, CASE WHEN size(recordLabels) > 0 THEN [recordLabels[0]] ELSE [] END as labelsToRemove`)
      .append(`CALL apoc.create.removeLabels(record, labelsToRemove) YIELD node as recordLabelRemove`)
      .append(
        `CALL apoc.create.addLabels(record, [coalesce(r.label, recordLabels[0])]) YIELD node as recordLabelUpdate`
      )
      .append(
        `WITH *, coalesce(apoc.convert.fromJsonMap(record.${RUSHDB_KEY_PROPERTIES_META}), {}) as existingTypesMap`
      )
      .append(
        `SET record.${RUSHDB_KEY_PROPERTIES_META} = apoc.convert.toJson(apoc.map.merge(existingTypesMap, typesMap))`
      )
      .append(`SET record += valuesMap`)
      .append(`WITH *`)
      .append(`UNWIND r.properties as prop`)
      .append(
        `MERGE (p:${RUSHDB_LABEL_PROPERTY} { name: prop.name, type: coalesce(prop.type, 'string'), projectId: $projectId, metadata: coalesce(prop.metadata, "")})`
      )
      .append(
        `ON CREATE SET p.created = coalesce(prop.created, time), p.id = prop.id, p.metadata = coalesce(prop.metadata, "")`
      )
      .append(`MERGE (p)-[rel:${RUSHDB_RELATION_VALUE}]->(record)`)

    return queryBuilder.getQuery()
  }

  deleteRecords(searchQuery?: SearchDto) {
    const { sortedQueryParts, parsedWhere } = buildQuery(searchQuery)
    const labelPart = singleLabelPart(searchQuery?.labels)

    const queryClauses = buildQueryClause({
      queryParts: sortedQueryParts,
      labelClause: buildLabelsClause(searchQuery?.labels)
    })

    const wherePart = parsedWhere.where ? `WHERE ${parsedWhere.where}` : ''
    const whereClause = `WITH ${parsedWhere.nodeAliases.join(', ')} ${wherePart}`.trim()
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`CALL apoc.periodic.iterate(`)
      .append(`'MATCH (record:${RUSHDB_LABEL_RECORD}${labelPart} { ${projectIdInline()} })`)
      .append(queryClauses.join(`\n`))
      .append(whereClause)
      .append(`RETURN record',`)
      .append(`"WITH record DETACH DELETE record",`)
      .append(`{ batchSize: 5000, params: { projectId: $projectId }, batchMode: "SINGLE", retries: 5 }`)
      .append(`) YIELD batch as b1, operations as o1`)

    return queryBuilder.getQuery()
  }

  createRelation(type?: string, direction: TRelationDirection = RELATION_DIRECTION_OUT) {
    const relationType = type ? type : RUSHDB_RELATION_DEFAULT
    let relationPart = `-[rel:${relationType}]-`

    if (direction === RELATION_DIRECTION_IN) {
      relationPart = '<' + relationPart
    }

    if (direction === RELATION_DIRECTION_OUT) {
      relationPart = relationPart + '>'
    }

    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`MATCH (record:${RUSHDB_LABEL_RECORD} { ${projectIdInline()}, ${RUSHDB_KEY_ID}: $id })`)
      .append(`UNWIND $targetIds AS targetId`)
      .append(
        `MATCH (targetRecord:${RUSHDB_LABEL_RECORD} { ${projectIdInline()}, ${RUSHDB_KEY_ID}: targetId })`
      )
      .append(`OPTIONAL MATCH (record)-[existingRel:${relationType}]-(targetRecord)`)
      .append(`DELETE existingRel`)
      .append(`MERGE (record)${relationPart}(targetRecord)`)
      .append(`SET rel += $properties`)

    return queryBuilder.getQuery()
  }

  deleteRelations(typeOrTypes?: MaybeArray<string>, direction?: TRelationDirection) {
    const buildMatchClause = (type?: string, index = 0) => {
      let relationPart = type ? `-[rel${index}:${type}]-` : `-[rel${index}]-`

      if (direction === RELATION_DIRECTION_IN) {
        relationPart = '<' + relationPart
      }

      if (direction === RELATION_DIRECTION_OUT) {
        relationPart = relationPart + '>'
      }

      return `(record)${relationPart}(targetRecord:${RUSHDB_LABEL_RECORD} { ${projectIdInline()}, ${RUSHDB_KEY_ID}: targetId })`
    }

    const matchClauses =
      isArray(typeOrTypes) ? typeOrTypes.map(buildMatchClause) : [buildMatchClause(typeOrTypes, 0)]
    const deleteClauses = matchClauses.map((_, index) => `rel${index}`)

    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`MATCH (record:${RUSHDB_LABEL_RECORD} { ${projectIdInline()}, ${RUSHDB_KEY_ID}: $id })`)
      .append(`UNWIND $targetIds AS targetId`)
      .append(`OPTIONAL MATCH ${matchClauses.join(' OPTIONAL MATCH ')}`)
      .append(`DELETE ${deleteClauses.join(', ')}`)

    return queryBuilder.getQuery()
  }

  sanitizeNeo4jIdentifier(value: string) {
    if (!value) {
      return ''
    }
    const normalized = String(value).trim()
    const stripped = normalized.replace(/[`\\]/g, '')
    const noSpaces = stripped.replace(/\s+/g, '_')
    return noSpaces.replace(/[^A-Za-z0-9_-]/g, '')
  }

  constructRelationshipQueryArguments({
    sourceLabel,
    sourceKey,
    targetLabel,
    targetKey,
    relationType,
    direction,
    sourceWhere,
    targetWhere,
    manyToMany
  }: {
    sourceLabel: string
    sourceKey?: string
    targetLabel: string
    targetKey?: string
    relationType?: string
    direction?: TRelationDirection
    sourceWhere?: Where
    targetWhere?: Where
    manyToMany?: boolean
  }) {
    const relType = relationType ? relationType : RUSHDB_RELATION_DEFAULT
    let relPattern = `-[rel:${relType}]-`
    if (direction === RELATION_DIRECTION_IN) {
      relPattern = '<' + relPattern
    }
    if (direction === RELATION_DIRECTION_OUT || !direction) {
      relPattern = relPattern + '>'
    }

    const safeSourceLabel = this.sanitizeNeo4jIdentifier(sourceLabel)
    const safeTargetLabel = this.sanitizeNeo4jIdentifier(targetLabel)
    const safeSourceKey = this.sanitizeNeo4jIdentifier(sourceKey)
    const safeTargetKey = this.sanitizeNeo4jIdentifier(targetKey)

    const buildAliasClauses = (where: Where | undefined, alias: string) => {
      if (!where || Object.keys(where).length === 0) {
        return { first: '', rest: [] as string[] }
      }
      const parsed = parseWhereClause(where, { nodeAlias: alias })
      const sorted = Object.keys(parsed.queryParts)
        .sort((a, b) => a.localeCompare(b))
        .map((k) => parsed.queryParts[k])
      const clauses: string[] = buildQueryClause({ queryParts: sorted })
      const [first, ...rest] = clauses
      return { first: first ?? '', rest }
    }

    const sClauses = buildAliasClauses(sourceWhere, 's')
    const tClauses = buildAliasClauses(targetWhere, 't')

    // Prepare pieces for embedding into apoc.periodic.iterate subqueries
    const sFirst = sClauses.first ? ` ${sClauses.first}` : ''
    const sRest = sClauses.rest.length ? ` ${sClauses.rest.join(' ')}` : ''
    const tFirst = tClauses.first ? ` ${tClauses.first}` : ''
    const tRest = tClauses.rest.length ? ` ${tClauses.rest.join(' ')}` : ''

    const hasSourceWhere = sourceWhere && Object.keys(sourceWhere).length > 0
    const hasTargetWhere = targetWhere && Object.keys(targetWhere).length > 0
    const hasJoinKeys = Boolean(safeSourceKey && safeTargetKey)

    // Safeguards. A key join is never cartesian, so `where` scoping is only
    // mandatory for the pure cross-product form of manyToMany.
    if (manyToMany) {
      if (!hasJoinKeys && (!hasSourceWhere || !hasTargetWhere)) {
        throw new BadRequestException(
          'manyToMany without join keys requires non-empty `where` filters for both source and target to avoid cartesian explosion. To join on a key, provide both source.key and target.key.'
        )
      }
    } else {
      if (!hasJoinKeys) {
        throw new BadRequestException('source.key and target.key are required unless manyToMany=true')
      }
    }

    const selfExclusion = safeSourceLabel === safeTargetLabel ? 'id(s) <> id(t)' : ''

    // The pair statement enumerates the source and target sets ONCE each and
    // emits distinct (s, t) pairs, so overall cost is O(|source| + |target| + pairs)
    // instead of re-matching the target label per source row. The target side is
    // wrapped in a CALL subquery both to build the join map up front and to scope
    // its `where` traversal aliases (parseSubQuery names them record<level>
    // regardless of the root alias, so source/target clauses would collide in a
    // shared scope).
    const targetMatch = `MATCH (t:${RUSHDB_LABEL_RECORD}:\`${safeTargetLabel}\` { ${projectIdInline()} })${tRest}${tFirst}`
    const sourceMatch = `MATCH (s:${RUSHDB_LABEL_RECORD}:\`${safeSourceLabel}\` { ${projectIdInline()} })${sFirst}${sRest}`

    let pairStatement: string
    if (hasJoinKeys) {
      // toString() bucketing is a pre-filter only — it can conflate values of
      // different types ("1" vs 1), so pairs are re-checked with the exact,
      // type-strict join predicate before being emitted.
      const pairFilter = [this.joinKeyPredicate('s', safeSourceKey, 't', safeTargetKey), selfExclusion]
        .filter(Boolean)
        .join(' AND ')
      pairStatement =
        `CALL { ${targetMatch} ` +
        `UNWIND ${this.asCypherList(`t.\`${safeTargetKey}\``)} AS targetValue ` +
        `WITH toString(targetValue) AS joinKey, collect(DISTINCT t) AS targets ` +
        `RETURN apoc.map.fromPairs(collect([joinKey, targets])) AS targetsByKey } ` +
        `${sourceMatch} ` +
        `UNWIND ${this.asCypherList(`s.\`${safeSourceKey}\``)} AS sourceValue ` +
        `WITH s, targetsByKey[toString(sourceValue)] AS candidates WHERE candidates IS NOT NULL ` +
        `UNWIND candidates AS t ` +
        `WITH s, t WHERE ${pairFilter} ` +
        `RETURN DISTINCT s, t`
    } else {
      pairStatement =
        `CALL { ${targetMatch} RETURN collect(DISTINCT t) AS targets } ` +
        `${sourceMatch} ` +
        `UNWIND targets AS t ` +
        (selfExclusion ? `WITH s, t WHERE ${selfExclusion} ` : '') +
        `RETURN DISTINCT s, t`
    }

    return { pairStatement, relPattern }
  }

  private joinKeyPredicate(sourceAlias: string, sourceKey: string, targetAlias: string, targetKey: string) {
    const sourceValue = `${sourceAlias}.\`${sourceKey}\``
    const targetValue = `${targetAlias}.\`${targetKey}\``
    const sourceValues = this.asCypherList(sourceValue)
    const targetValues = this.asCypherList(targetValue)

    return `any(sourceValue IN ${sourceValues} WHERE sourceValue IN ${targetValues})`
  }

  private asCypherList(value: string) {
    // Native type predicate (Neo4j 5.9+) — apoc.meta.cypher.type here cost a
    // procedure call per evaluated row in the bulk-join hot path.
    return `CASE WHEN ${value} IS NULL THEN [] WHEN ${value} IS :: LIST<ANY> THEN ${value} ELSE [${value}] END`
  }

  createRelationsByKeys(payload: {
    sourceLabel: string
    sourceKey?: string
    targetLabel: string
    targetKey?: string
    relationType?: string
    direction?: TRelationDirection
    sourceWhere?: Where
    targetWhere?: Where
    manyToMany?: boolean
  }) {
    const { pairStatement, relPattern } = this.constructRelationshipQueryArguments(payload)

    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append('CALL apoc.periodic.iterate(')
      .append(`'${pairStatement}',`)
      .append(
        `'UNWIND $_batch AS row WITH row.s AS s, row.t AS t MERGE (s)${relPattern}(t) SET rel += $relationshipProperties RETURN count(*)',`
      )
      .append(
        `{ batchSize: 5000, params: { projectId: $projectId, relationshipProperties: $relationshipProperties }, batchMode: "BATCH_SINGLE", retries: 5 }`
      )
      .append(')')
      .append('YIELD total, committedOperations, failedOperations, errorMessages')
      .append('RETURN total, committedOperations, failedOperations, errorMessages')

    return queryBuilder.getQuery()
  }

  /**
   * Counts (up to `limit`) the distinct record pairs a join pattern would connect,
   * via the same pair statement createRelationsByKeys executes — so a probe result
   * is exact evidence of what applying the pattern would do. LIMIT caps the pairs
   * pulled from the lazy pair pipeline, bounding probe cost on large graphs.
   */
  countRelationCandidatesByKeys(payload: {
    sourceLabel: string
    sourceKey?: string
    targetLabel: string
    targetKey?: string
    sourceWhere?: Where
    targetWhere?: Where
    limit: number
  }) {
    const { pairStatement } = this.constructRelationshipQueryArguments(payload)
    const limit = Math.max(1, Math.floor(payload.limit))

    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`CALL { ${pairStatement} }`)
      .append(`WITH s, t LIMIT ${limit}`)
      .append('RETURN count(*) AS matchCount')

    return queryBuilder.getQuery()
  }

  retypeRelationsByLabels({
    sourceLabel,
    targetLabel,
    sourceRelationType,
    targetRelationType,
    direction
  }: {
    sourceLabel: string
    targetLabel: string
    sourceRelationType?: string
    targetRelationType?: string
    direction?: TRelationDirection
  }) {
    const safeSourceLabel = this.sanitizeNeo4jIdentifier(sourceLabel)
    const safeTargetLabel = this.sanitizeNeo4jIdentifier(targetLabel)
    const sourceRelType = sourceRelationType ? sourceRelationType : RUSHDB_RELATION_DEFAULT
    const targetRelType = targetRelationType ? targetRelationType : RUSHDB_RELATION_DEFAULT
    const relPattern =
      direction === RELATION_DIRECTION_IN ? `<-[newRel:${targetRelType}]-` : `-[newRel:${targetRelType}]->`
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append('CALL apoc.periodic.iterate(')
      .append(
        `'MATCH (s:${RUSHDB_LABEL_RECORD}:\`${safeSourceLabel}\` { ${projectIdInline()} })-[:${sourceRelType}]-(t:${RUSHDB_LABEL_RECORD}:\`${safeTargetLabel}\` { ${projectIdInline()} }) RETURN DISTINCT s, t',`
      )
      .append(
        `'MERGE (s)${relPattern}(t) WITH s, t OPTIONAL MATCH (s)-[old:${sourceRelType}]-(t) DELETE old RETURN count(*)',`
      )
      .append(`{ batchSize: 5000, params: { projectId: $projectId }, batchMode: "SINGLE", retries: 5 }`)
      .append(')')
      .append('YIELD total, committedOperations, failedOperations, errorMessages')
      .append('RETURN total, committedOperations, failedOperations, errorMessages')

    return queryBuilder.getQuery()
  }

  deleteRelationsByKeys(payload: {
    sourceLabel: string
    sourceKey?: string
    targetLabel: string
    targetKey?: string
    relationType?: string
    direction?: TRelationDirection
    sourceWhere?: Where
    targetWhere?: Where
    manyToMany?: boolean
  }) {
    const { pairStatement, relPattern } = this.constructRelationshipQueryArguments(payload)

    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append('CALL apoc.periodic.iterate(')
      .append(`'${pairStatement}',`)
      .append(
        `'UNWIND $_batch AS row WITH row.s AS s, row.t AS t OPTIONAL MATCH (s)${relPattern}(t) DELETE rel RETURN count(*)',`
      )
      .append(`{ batchSize: 5000, params: { projectId: $projectId }, batchMode: "BATCH_SINGLE", retries: 5 }`)
      .append(')')
      .append('YIELD total, committedOperations, failedOperations, errorMessages')
      .append('RETURN total, committedOperations, failedOperations, errorMessages')

    return queryBuilder.getQuery()
  }

  deleteRelationsByLabels({
    sourceLabel,
    targetLabel,
    relationType
  }: {
    sourceLabel: string
    targetLabel: string
    relationType?: string
  }) {
    const safeSourceLabel = this.sanitizeNeo4jIdentifier(sourceLabel)
    const safeTargetLabel = this.sanitizeNeo4jIdentifier(targetLabel)
    const relType = relationType ? relationType : RUSHDB_RELATION_DEFAULT
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append('CALL apoc.periodic.iterate(')
      .append(
        `'MATCH (s:${RUSHDB_LABEL_RECORD}:\`${safeSourceLabel}\` { ${projectIdInline()} })-[rel:${relType}]-(t:${RUSHDB_LABEL_RECORD}:\`${safeTargetLabel}\` { ${projectIdInline()} }) RETURN rel',`
      )
      .append(`'DELETE rel RETURN count(*)',`)
      .append(`{ batchSize: 5000, params: { projectId: $projectId }, batchMode: "SINGLE", retries: 5 }`)
      .append(')')
      .append('YIELD total, committedOperations, failedOperations, errorMessages')
      .append('RETURN total, committedOperations, failedOperations, errorMessages')

    return queryBuilder.getQuery()
  }
}
