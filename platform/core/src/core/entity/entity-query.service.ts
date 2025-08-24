import { Injectable } from '@nestjs/common'

import { isArray } from '@/common/utils/isArray'
import { toBoolean } from '@/common/utils/toBolean'
import {
  RUSHDB_KEY_ID,
  RUSHDB_LABEL_PROPERTY,
  RUSHDB_KEY_PROJECT_ID,
  RUSHDB_KEY_PROPERTIES_META,
  RUSHDB_LABEL_RECORD,
  RUSHDB_RELATION_VALUE,
  RUSHDB_RELATION_DEFAULT,
  DEFAULT_RECORD_ALIAS
} from '@/core/common/constants'
import { MaybeArray, Where } from '@/core/common/types'
import { RELATION_DIRECTION_IN, RELATION_DIRECTION_OUT } from '@/core/entity/entity.constants'
import { TRelationDirection } from '@/core/entity/entity.types'
import { SearchDto } from '@/core/search/dto/search.dto'
import { buildAggregation } from '@/core/search/parser'
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
import { projectIdInline } from '@/core/search/parser/projectIdInline'
import { singleLabelPart } from '@/core/search/parser/singleLabelPart'
import { QueryBuilder } from '@/database/QueryBuilder'

import { label } from '../search/parser/pickRecordLabel'

@Injectable()
export class EntityQueryService {
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
      .append(`RETURN record {.*, ${label()}} as data`)

    return queryBuilder.getQuery()
  }

  getEntity() {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`MATCH (record:${RUSHDB_LABEL_RECORD} { ${RUSHDB_KEY_ID}: $id, ${projectIdInline()} })`)
      .append(`RETURN record {.*, ${label()}} as data`)

    return queryBuilder.getQuery()
  }

  editRecord() {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`WITH $record as r, datetime() as time`)
      .append(`MATCH (record:${RUSHDB_LABEL_RECORD} { ${RUSHDB_KEY_ID}: r.id })`)
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
      queryBuilder.append(`RETURN collect(DISTINCT record {.*, ${label()}}) as data`)
    }

    return queryBuilder.getQuery()
  }

  findRecords({ id, searchQuery }: { searchQuery?: SearchDto; id?: string }) {
    const relatedQueryPart = buildRelatedQueryPart(id)
    const labelPart = singleLabelPart(searchQuery?.labels)

    const { queryClauses, sortedQueryParts, parsedWhere, aliasesMap } = buildQuery(searchQuery)

    const pagination = buildPagination(searchQuery)
    const orderByAggregatedField = isOrderByAggregatedField(searchQuery)
    const sortParams = sort(searchQuery.orderBy, orderByAggregatedField ? null : DEFAULT_RECORD_ALIAS)

    const { withPart: aggregateProjections, recordPart: returnPart } = buildAggregation(
      searchQuery?.aggregate,
      aliasesMap
    )

    // convert a clause array to string
    const normalizedQueryClauses = queryClauses
      .map((clause, index) => {
        if (!orderByAggregatedField && index === 0) {
          return `${clause} ${sortParams} ${pagination}`.trim()
        } else {
          return clause
        }
      })
      // @FYI: Keep it in this order
      .filter(toBoolean)
      .join(`\n`)

    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`MATCH ${relatedQueryPart}(record:${RUSHDB_LABEL_RECORD}${labelPart} { ${projectIdInline()} })`)
      .append(normalizedQueryClauses)

    if (parsedWhere.nodeAliases.filter(toBoolean).length > 1) {
      const wherePart = parsedWhere.where ? `WHERE ${parsedWhere.where}` : ''

      queryBuilder.append(`WITH ${parsedWhere.nodeAliases.join(', ')} ${wherePart}`.trim())
    }

    queryBuilder.append(aggregateProjections)

    if (orderByAggregatedField) {
      queryBuilder.append(`${sortParams} ${pagination}`)
    }

    queryBuilder.append(`RETURN ${returnPart}`)

    return queryBuilder.getQuery()
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

    if (parsedWhere.nodeAliases.filter(toBoolean).length > 1) {
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

      if (parsedWhere.nodeAliases.filter(toBoolean).length > 1) {
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
        `CALL apoc.create.relationship(source, coalesce(relation.type, '${RUSHDB_RELATION_DEFAULT}'), {}, target) YIELD rel`
      )
      .append(`RETURN rel`)

    return queryBuilder.getQuery()
  }

  getRecordRelations({
    id,
    searchQuery,
    pagination
  }: {
    id?: string
    searchQuery?: SearchDto
    pagination?: Pick<SearchDto, 'skip' | 'limit'>
  }) {
    const relatedQueryPart = buildRelatedQueryPart(id)
    const labelPart = singleLabelPart(searchQuery?.labels)

    const { sortedQueryParts, parsedWhere, queryClauses: rawQueryClauses } = buildQuery(searchQuery)

    const queryClauses = buildQueryClause({
      queryParts: sortedQueryParts,
      labelClause: buildLabelsClause(searchQuery?.labels)
    })

    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`MATCH ${relatedQueryPart}(record:${RUSHDB_LABEL_RECORD}${labelPart} { ${projectIdInline()} })`)
      .append(toBoolean(id) ? queryClauses.join(`\n`) : rawQueryClauses.join(`\n`))

    if (parsedWhere.nodeAliases.filter(toBoolean).length > 1) {
      const wherePart = parsedWhere.where ? `WHERE ${parsedWhere.where}` : ''

      queryBuilder.append(`WITH ${parsedWhere.nodeAliases.join(', ')} ${wherePart}`.trim())
    }

    if (!toBoolean(id)) {
      queryBuilder.append(`MATCH (record)-[rel]-(neighbor:${RUSHDB_LABEL_RECORD})`)
      queryBuilder.append(buildPagination(pagination))
    } else {
      queryBuilder.append(`MATCH (neighbor:${RUSHDB_LABEL_RECORD})-[rel]-(record:${RUSHDB_LABEL_RECORD})`)
    }

    queryBuilder.append(`RETURN DISTINCT `)
    queryBuilder.append(`CASE`)
    queryBuilder.append(`  WHEN startNode(rel) = record THEN {`)
    queryBuilder.append(`    sourceId: record.${RUSHDB_KEY_ID}, ${label('record', 'sourceLabel')},`)
    queryBuilder.append(`    targetId: neighbor.${RUSHDB_KEY_ID}, ${label('neighbor', 'targetLabel')},`)
    queryBuilder.append(`    type: type(rel)`)
    queryBuilder.append(`  }`)
    queryBuilder.append(`  ELSE {`)
    queryBuilder.append(`    sourceId: neighbor.${RUSHDB_KEY_ID}, ${label('neighbor', 'sourceLabel')},`)
    queryBuilder.append(`    targetId: record.${RUSHDB_KEY_ID}, ${label('record', 'targetLabel')},`)
    queryBuilder.append(`    type: type(rel)`)
    queryBuilder.append(`  }`)
    queryBuilder.append(`END AS relation`)

    return queryBuilder.getQuery()
  }

  getRecordRelationsCount({ id, searchQuery }: { id?: string; searchQuery?: SearchDto }) {
    const relatedQueryPart = buildRelatedQueryPart(id)
    const labelPart = singleLabelPart(searchQuery?.labels)

    const { sortedQueryParts, parsedWhere, queryClauses: rawQueryClauses } = buildQuery(searchQuery)

    const queryClauses = buildQueryClause({
      queryParts: sortedQueryParts,
      labelClause: buildLabelsClause(searchQuery?.labels)
    })

    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`MATCH ${relatedQueryPart}(record:${RUSHDB_LABEL_RECORD}${labelPart} { ${projectIdInline()} })`)
      .append(toBoolean(id) ? queryClauses.join(`\n`) : rawQueryClauses.join(`\n`))

    if (parsedWhere.nodeAliases.filter(toBoolean).length > 1) {
      const wherePart = parsedWhere.where ? `WHERE ${parsedWhere.where}` : ''

      queryBuilder.append(`WITH ${parsedWhere.nodeAliases.join(', ')} ${wherePart}`.trim())
    }

    if (!toBoolean(id)) {
      queryBuilder.append(`MATCH (record)-[rel]-(target:${RUSHDB_LABEL_RECORD})`)
    }

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
        `MERGE (p:${RUSHDB_LABEL_PROPERTY} { name: prop.name, type: prop.type, projectId: $projectId, metadata: coalesce(prop.metadata, "")})`
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

  createRelationsByKeys({
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
    sourceKey: string
    targetLabel: string
    targetKey: string
    relationType?: string
    direction?: TRelationDirection
    sourceWhere?: Where
    targetWhere?: Where
    manyToMany?: boolean
  }) {
    const relType = relationType ? relationType : RUSHDB_RELATION_DEFAULT
    let arrow = `-[:${relType}]-`
    if (direction === RELATION_DIRECTION_IN) {
      arrow = '<' + arrow
    }
    if (direction === RELATION_DIRECTION_OUT || !direction) {
      arrow = arrow + '>'
    }

    // Defensive sanitization for identifiers used in backticked contexts (labels/keys).
    // Neo4j does not allow parameterizing identifiers, so we strictly filter to a safe subset.
    // Use the class private method for sanitization.

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
    const tRest = tClauses.rest.length ? ` ${tClauses.rest.join(' ')}` : ''
    const tFirstStartsWithWhere = /^\s*WHERE\b/i.test(tClauses.first)
    const tFirstNoWhere = tFirstStartsWithWhere ? tClauses.first.replace(/^\s*WHERE\s+/i, '') : ''

    const hasSourceWhere = sourceWhere && Object.keys(sourceWhere).length > 0
    const hasTargetWhere = targetWhere && Object.keys(targetWhere).length > 0
    const hasJoinKeys = Boolean(safeSourceKey && safeTargetKey)

    // Safeguards
    if (manyToMany) {
      if (!hasSourceWhere || !hasTargetWhere) {
        throw new Error(
          'manyToMany requires non-empty `where` filters for both source and target to avoid cartesian explosion'
        )
      }
    } else {
      if (!hasJoinKeys) {
        throw new Error('source.key and target.key are required unless manyToMany=true')
      }
    }

    // Build combined WHERE depending on whether join keys exist.
    let combinedWhere = ''
    if (hasJoinKeys) {
      const eqClause = `s.\`${safeSourceKey}\` = t.\`${safeTargetKey}\``
      combinedWhere = tFirstNoWhere ? `WHERE ${eqClause} AND ${tFirstNoWhere}` : `WHERE ${eqClause}`
    } else {
      // manyToMany path: rely solely on target's where (already required to be non-empty)
      combinedWhere = tFirstNoWhere ? `WHERE ${tFirstNoWhere}` : ''
    }

    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append('CALL apoc.periodic.iterate(')
      .append(
        `'MATCH (s:${RUSHDB_LABEL_RECORD}:\`${safeSourceLabel}\` { ${projectIdInline()} })${sFirst}${sRest} RETURN s',`
      )
      .append(
        `'WITH s MATCH (t:${RUSHDB_LABEL_RECORD}:\`${safeTargetLabel}\` { ${projectIdInline()} })${tRest} ${combinedWhere} MERGE (s)${arrow}(t) RETURN count(*)',`
      )
      .append(`{ batchSize: 5000, params: { projectId: $projectId }, batchMode: "SINGLE", retries: 5 }`)
      .append(')')

    return queryBuilder.getQuery()
  }

  deleteRelationsByKeys({
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
    // Build a relation pattern with variable name for deletion
    let relPattern = `-[rel:${relType}]-`
    if (direction === RELATION_DIRECTION_IN) {
      relPattern = '<' + relPattern
    }
    if (direction === RELATION_DIRECTION_OUT || !direction) {
      relPattern = relPattern + '>'
    }

    // Defensive sanitization for identifiers used in backticked contexts (labels/keys).
    const sanitizeNeo4jIdentifier = (value: string) => {
      if (!value) {
        return ''
      }
      const normalized = String(value).trim()
      const stripped = normalized.replace(/[`\\]/g, '')
      const noSpaces = stripped.replace(/\s+/g, '_')
      const safe = noSpaces.replace(/[^A-Za-z0-9_-]/g, '')
      return safe
    }

    const safeSourceLabel = sanitizeNeo4jIdentifier(sourceLabel)
    const safeTargetLabel = sanitizeNeo4jIdentifier(targetLabel)
    const safeSourceKey = sanitizeNeo4jIdentifier(sourceKey)
    const safeTargetKey = sanitizeNeo4jIdentifier(targetKey)

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
    const tRest = tClauses.rest.length ? ` ${tClauses.rest.join(' ')}` : ''
    const tFirstStartsWithWhere = /^\s*WHERE\b/i.test(tClauses.first)
    const tFirstNoWhere = tFirstStartsWithWhere ? tClauses.first.replace(/^\s*WHERE\s+/i, '') : ''

    const hasSourceWhere = sourceWhere && Object.keys(sourceWhere).length > 0
    const hasTargetWhere = targetWhere && Object.keys(targetWhere).length > 0
    const hasJoinKeys = Boolean(safeSourceKey && safeTargetKey)

    // Safeguards
    if (manyToMany) {
      if (!hasSourceWhere || !hasTargetWhere) {
        throw new Error(
          'manyToMany requires non-empty `where` filters for both source and target to avoid cartesian explosion'
        )
      }
    } else {
      if (!hasJoinKeys) {
        throw new Error('source.key and target.key are required unless manyToMany=true')
      }
    }

    // Build combined WHERE depending on whether join keys exist.
    let combinedWhere = ''
    if (hasJoinKeys) {
      const eqClause = `s.\`${safeSourceKey}\` = t.\`${safeTargetKey}\``
      combinedWhere = tFirstNoWhere ? `WHERE ${eqClause} AND ${tFirstNoWhere}` : `WHERE ${eqClause}`
    } else {
      // manyToMany path: rely solely on target's where (already required to be non-empty)
      combinedWhere = tFirstNoWhere ? `WHERE ${tFirstNoWhere}` : ''
    }

    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append('CALL apoc.periodic.iterate(')
      .append(
        `'MATCH (s:${RUSHDB_LABEL_RECORD}:\`${safeSourceLabel}\` { ${projectIdInline()} })${sFirst}${sRest} RETURN s',`
      )
      .append(
        `'WITH s MATCH (t:${RUSHDB_LABEL_RECORD}:\`${safeTargetLabel}\` { ${projectIdInline()} })${tRest} ${combinedWhere} OPTIONAL MATCH (s)${relPattern}(t) DELETE rel RETURN count(*)',`
      )
      .append(`{ batchSize: 5000, params: { projectId: $projectId }, batchMode: "SINGLE", retries: 5 }`)
      .append(')')

    return queryBuilder.getQuery()
  }
}
