import { Injectable } from '@nestjs/common'

import { QueryBuilder } from '@/common/QueryBuilder'
import { isArray } from '@/common/utils/isArray'
import { toBoolean } from '@/common/utils/toBolean'
import {
  RUSHDB_KEY_ID,
  RUSHDB_LABEL_PROPERTY,
  RUSHDB_KEY_PROJECT_ID,
  RUSHDB_KEY_PROPERTIES_META,
  RUSHDB_LABEL_RECORD,
  RUSHDB_RELATION_VALUE,
  RUSHDB_RELATION_DEFAULT
} from '@/core/common/constants'
import { MaybeArray } from '@/core/common/types'
import { UpsertEntityDto } from '@/core/entity/dto/upsert-entity.dto'
import { RELATION_DIRECTION_IN, RELATION_DIRECTION_OUT } from '@/core/entity/entity.constants'
import { TRelationDirection } from '@/core/entity/entity.types'
import { SearchDto } from '@/core/search/dto/search.dto'
import { buildAggregation } from '@/core/search/parser'
import {
  buildLabelsClause,
  buildPagination,
  buildQuery,
  buildQueryClause
} from '@/core/search/parser/buildQuery'
import { buildRelatedQueryPart } from '@/core/search/parser/buildRelatedRecordQueryPart'
import { projectIdInline } from '@/core/search/parser/projectIdInline'
import { singleLabelPart } from '@/core/search/parser/singleLabelPart'

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

  upsert(record: UpsertEntityDto & { id?: string; created: string }, mergeBy?: Array<string>) {
    const queryBuilder = new QueryBuilder()

    // Build the match criteria dynamically based on mergeBy or all properties
    const propertiesCriteria = record.properties
      .map((property) => {
        if ((toBoolean(mergeBy?.length) && mergeBy?.includes(property.name)) || !toBoolean(mergeBy)) {
          return `${property.name}: ${property.value}`
        }
      })
      .filter(toBoolean)
      .join(', ')

    queryBuilder.append(`WITH $record as r, datetime() as time`)
    console.log(propertiesCriteria)
    // Merge based on match criteria - use id if provided, otherwise use dynamic matching
    if (propertiesCriteria) {
      queryBuilder.append(
        `MERGE (record:${RUSHDB_LABEL_RECORD} { ${projectIdInline()}, ${propertiesCriteria} })`
      )
    } else {
      queryBuilder.append(`MERGE (record:${RUSHDB_LABEL_RECORD} { ${projectIdInline()} })`)
    }

    // Handle ON CREATE - set the ID if not already set
    queryBuilder
      .append(`ON CREATE SET record.${RUSHDB_KEY_ID} = r.id`)
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

    const { withPart: aggregateProjections, recordPart: returnPart } = buildAggregation(
      searchQuery?.aggregate,
      aliasesMap
    )

    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`MATCH ${relatedQueryPart}(record:${RUSHDB_LABEL_RECORD}${labelPart} { ${projectIdInline()} })`)
      .append(queryClauses)

    if (sortedQueryParts.length > 1) {
      queryBuilder.append(`WITH ${parsedWhere.nodeAliases.join(', ')}`).append(`WHERE ${parsedWhere.where}`)
    }

    queryBuilder.append(aggregateProjections).append(`RETURN ${returnPart}`)

    console.log(queryBuilder.getQuery())
    return queryBuilder.getQuery()
  }

  getRecordsCount({ id, searchQuery }: { id?: string; searchQuery?: SearchDto }) {
    const relatedQueryPart = buildRelatedQueryPart(id)
    const labelPart = singleLabelPart(searchQuery?.labels)

    const { sortedQueryParts, parsedWhere } = buildQuery(searchQuery)

    const queryClauses = buildQueryClause({
      queryParts: sortedQueryParts,
      // Explicitly omitting sort and pagination for count query
      sortParams: '',
      pagination: '',
      labelClause: buildLabelsClause(searchQuery?.labels)
    })

    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`MATCH ${relatedQueryPart}(record:${RUSHDB_LABEL_RECORD}${labelPart} { ${projectIdInline()} })`)
      .append(queryClauses)

    if (sortedQueryParts.length > 1) {
      queryBuilder.append(`WITH ${parsedWhere.nodeAliases.join(', ')}`).append(`WHERE ${parsedWhere.where}`)
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
        // Explicitly omitting sort and pagination for count query
        sortParams: '',
        pagination: '',
        labelClause: buildLabelsClause(searchQuery?.labels)
      })

      queryBuilder
        .append(`MATCH (record:${RUSHDB_LABEL_RECORD}${labelPart} { ${projectIdInline()} })`)
        .append(queryClauses)

      if (sortedQueryParts.length > 1) {
        queryBuilder.append(`WITH ${parsedWhere.nodeAliases.join(', ')}`).append(`WHERE ${parsedWhere.where}`)
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
      // Explicitly omitting sort and pagination for count query
      sortParams: '',
      pagination: '',
      labelClause: buildLabelsClause(searchQuery?.labels)
    })

    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`MATCH (record:${RUSHDB_LABEL_RECORD} { ${projectIdInline()} })`)
      .append(queryClauses)
      .append(`WITH ${parsedWhere.nodeAliases.join(', ')} WHERE ${parsedWhere.where}`)
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
      // Explicitly omitting sort and pagination for id-targeted query
      sortParams: '',
      pagination: '',
      labelClause: buildLabelsClause(searchQuery?.labels)
    })

    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`MATCH ${relatedQueryPart}(record:${RUSHDB_LABEL_RECORD}${labelPart} { ${projectIdInline()} })`)
      .append(toBoolean(id) ? queryClauses : rawQueryClauses)

    if (sortedQueryParts.length > 1) {
      queryBuilder.append(`WITH ${parsedWhere.nodeAliases.join(', ')}`).append(`WHERE ${parsedWhere.where}`)
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
      // Explicitly omitting sort and pagination for id-targeted query
      sortParams: '',
      pagination: '',
      labelClause: buildLabelsClause(searchQuery?.labels)
    })

    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`MATCH ${relatedQueryPart}(record:${RUSHDB_LABEL_RECORD}${labelPart} { ${projectIdInline()} })`)
      .append(toBoolean(id) ? queryClauses : rawQueryClauses)

    if (sortedQueryParts.length > 1) {
      queryBuilder.append(`WITH ${parsedWhere.nodeAliases.join(', ')}`).append(`WHERE ${parsedWhere.where}`)
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
      // Explicitly omitting sort and pagination for count query
      sortParams: '',
      pagination: '',
      labelClause: buildLabelsClause(searchQuery?.labels)
    })

    const whereClause = `WITH ${parsedWhere.nodeAliases.join(', ')} WHERE ${parsedWhere.where}`
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`CALL apoc.periodic.iterate(`)
      .append(`'MATCH (record:${RUSHDB_LABEL_RECORD}${labelPart} { ${projectIdInline()} })`)
      .append(queryClauses)
      .append(whereClause)
      .append(`RETURN record',`)
      .append(`"WITH record DETACH DELETE record",`)
      .append(`{ batchSize: 5000, params: { projectId: $projectId }, batchMode: "SINGLE", retries: 5 }`)
      .append(`) YIELD batch as b1, operations as o1`)

    return queryBuilder.getQuery()
  }

  deleteRecordsByIds() {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`CALL apoc.periodic.iterate(`)
      .append(`"MATCH (record:${RUSHDB_LABEL_RECORD} { ${projectIdInline()} })`)
      .append(`WITH *, collect(record.${RUSHDB_KEY_ID}) as recordIds`)
      .append(`WHERE any(id IN recordIds WHERE id IN $idsToDelete)`)
      .append(`RETURN record",`)
      .append(`"WITH record DETACH DELETE record",`)
      .append(
        `{ batchSize: 5000, params: { projectId: $projectId, idsToDelete: $idsToDelete }, batchMode: "SINGLE", retries: 5 }`
      )
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
}
