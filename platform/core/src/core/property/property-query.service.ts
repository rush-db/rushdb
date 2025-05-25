import { Injectable } from '@nestjs/common'

import { QueryBuilder } from '@/common/QueryBuilder'
import {
  RUSHDB_KEY_ID,
  RUSHDB_KEY_PROPERTIES_META,
  RUSHDB_LABEL_PROPERTY,
  RUSHDB_LABEL_RECORD,
  RUSHDB_RELATION_VALUE
} from '@/core/common/constants'
import { SearchDto } from '@/core/search/dto/search.dto'
import { buildLabelsClause, buildQuery, buildQueryClause } from '@/core/search/parser/buildQuery'
import { pagination } from '@/core/search/parser/pagination'
import { projectIdInline } from '@/core/search/parser/projectIdInline'
import { singleLabelPart } from '@/core/search/parser/singleLabelPart'
import { SORT_ASC, SORT_DESC } from '@/core/search/search.constants'
import { TSearchSortDirection } from '@/core/search/search.types'

@Injectable()
export class PropertyQueryService {
  private matchPart() {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`MATCH (record:${RUSHDB_LABEL_RECORD}),`)
      .append(
        `(record)<-[relation:${RUSHDB_RELATION_VALUE}]-(property:${RUSHDB_LABEL_PROPERTY} { id: $target })`
      )

    return queryBuilder.getQuery()
  }

  deletePropertyQuery() {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(this.matchPart())
      .append(`DELETE relation`)
      .append(`WITH *, collect(property.name) AS propertyKeyToDelete,`)
      .append(`apoc.convert.fromJsonMap(record.${RUSHDB_KEY_PROPERTIES_META}) AS recordPropertiesMetaMap`)
      .append(`CALL apoc.create.removeProperties(record, propertyKeyToDelete) YIELD node as _result0`)
      .append(
        `WITH *, apoc.map.removeKey(recordPropertiesMetaMap, property.name) as updatedRecordPropertiesMetaMap`
      )
      .append(
        `SET record.${RUSHDB_KEY_PROPERTIES_META} = apoc.convert.toJson(updatedRecordPropertiesMetaMap)`
      )

    return queryBuilder.getQuery()
  }

  updateField() {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(this.matchPart())
      .append(`DELETE relation`)
      .append(`WITH *, apoc.convert.toList($newValue) as valueRaw`)
      .append(`WITH *, range(0, size(valueRaw)-1) AS valueIndexesList`)
      .append(`UNWIND valueIndexesList AS index`)
      .append(
        `WITH property, record, index, CASE property.type WHEN "datetime" THEN datetime(valueRaw[index]) ELSE valueRaw[index] END as value,`
      )
      .append(`apoc.map.setEntry(record, property.name, valueRaw) AS resultMeta`)
      .append(`SET record += resultMeta`)
      .append(`MERGE (property)-[rel:${RUSHDB_RELATION_VALUE}]->(record)`)

    return queryBuilder.getQuery()
  }

  updateFieldNode() {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(
        `MATCH (record:${RUSHDB_LABEL_RECORD})<-[value:${RUSHDB_RELATION_VALUE}]-(property:${RUSHDB_LABEL_PROPERTY} {id: $target})`
      )
      .append(`WITH *, CASE WHEN $newName IS NOT NULL THEN $newName ELSE property.name END as nameToSet,`)
      .append(`CASE WHEN $newType IS NOT NULL THEN $newType ELSE property.type END as typeToSet`)
      .append(`WITH *, collect(property.name) AS propertyKeyToDelete, `)
      .append(`apoc.convert.fromJsonMap(record.${RUSHDB_KEY_PROPERTIES_META}) AS recordPropertiesMetaMap`)
      .append(`CALL apoc.create.removeProperties(record, propertyKeyToDelete) YIELD node as _result0`)
      .append(
        `WITH *, apoc.map.removeKey(recordPropertiesMetaMap, property.name) as cleanRecordPropertiesMetaMap`
      )
      .append(`WITH *, apoc.map.setEntry(cleanRecordPropertiesMetaMap, nameToSet, typeToSet) AS updatedMeta`)
      .append(`WITH *, apoc.convert.toList(toString(value.value)) as valueRaw`)
      .append(`WITH *, range(0, size(valueRaw)-1) AS valueIndexesList`)
      .append(`SET record.${RUSHDB_KEY_PROPERTIES_META} = apoc.convert.toJson(updatedMeta)`)
      .append(`WITH *`)
      .append(`UNWIND valueIndexesList AS index`)
      .append(
        `WITH nameToSet, typeToSet, property, record, index, CASE property.type WHEN "datetime" THEN datetime(valueRaw[index]) ELSE valueRaw[index] END as value,`
      )
      .append(`apoc.map.setEntry(record, nameToSet, valueRaw) AS resultMeta`)
      .append(`SET record += resultMeta`)
      .append(`MERGE (property)-[rel:${RUSHDB_RELATION_VALUE}]->(record)`)
      .append(`SET property.name = nameToSet`)
      .append(`SET property.type = typeToSet`)

    return queryBuilder.getQuery()
  }

  mergeFieldMeta() {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(
        `MATCH (record:${RUSHDB_LABEL_RECORD})<-[value:${RUSHDB_RELATION_VALUE}]-(property:${RUSHDB_LABEL_PROPERTY} { id: $target })`
      )
      .append(`WITH *, CASE WHEN $newType IS NOT NULL THEN $newType ELSE property.type END as typeToSet`)

      .append(`OPTIONAL MATCH (conflictProperty:${RUSHDB_LABEL_PROPERTY} { id: $conflictNodeId })`)
      .append(`OPTIONAL MATCH (conflictProperty)-[existingValue:${RUSHDB_RELATION_VALUE}]->(record)`)
      .append(`WITH *, collect(value.value) as valuesToAttach, collect(existingValue.value) as sourceValues`)
      .append(`DELETE existingValue`)
      .append(`WITH *`)
      .append(`CALL {`)
      .append(`WITH *`)
      .append(`UNWIND valuesToAttach as valueToAttach`)
      .append(`RETURN CASE WHEN (typeToSet = "string" AND property.type <> "string")`)
      .append(`THEN toString(valueToAttach)`)
      .append(`ELSE valueToAttach`)
      .append(`END as rawValue`)
      .append(`}`)
      .append(
        `WITH conflictProperty, typeToSet, property, record, sourceValues, collect(rawValue) as rawValues`
      )
      .append(`CALL {`)
      .append(`WITH *`)
      .append(`UNWIND (CASE sourceValues WHEN [] THEN [null] ELSE sourceValues END) as rawSourceValue`)
      .append(`RETURN rawSourceValue`)
      .append(`}`)
      .append(
        `WITH conflictProperty, typeToSet, property, record, rawValues, collect(rawSourceValue) as sourceRawValues`
      )
      .append(`WITH conflictProperty, typeToSet, property, record, sourceRawValues + rawValues as valueRaw`)
      .append(`WITH conflictProperty,typeToSet, property, record, valueRaw,`)
      .append(`range(0, size(valueRaw)-1) AS valueIndexesList`)
      .append(`UNWIND valueIndexesList AS index`)
      .append(`WITH conflictProperty, typeToSet, property, record, index, valueRaw[index] as value,`)
      .append(`apoc.map.setEntry(record, conflictProperty.name, valueRaw) AS resultMeta`)
      .append(`SET record += resultMeta`)
      .append(
        `WITH *, apoc.convert.fromJsonMap(record.${RUSHDB_KEY_PROPERTIES_META}) AS recordPropertiesMetaMap`
      )
      .append(
        `WITH *, apoc.map.setEntry(recordPropertiesMetaMap, conflictProperty.name, typeToSet) AS updatedMeta`
      )
      .append(`SET record.${RUSHDB_KEY_PROPERTIES_META} = apoc.convert.toJson(updatedMeta)`)
      .append(`MERGE (conflictProperty)-[rel:${RUSHDB_RELATION_VALUE}]->(record)`)

    return queryBuilder.getQuery()
  }

  getProjectProperties() {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`MATCH (property:${RUSHDB_LABEL_PROPERTY} { projectId: $id } )`)
      .append(`RETURN collect(DISTINCT property { .id, .metadata, .name, .type, .projectId }) as properties`)

    return queryBuilder.getQuery()
  }

  getPropertyValues({
    searchQuery
  }: {
    searchQuery?: SearchDto & { query?: string; orderBy?: TSearchSortDirection }
  }) {
    const sort = [SORT_ASC, SORT_DESC].includes(searchQuery.orderBy) ? searchQuery.orderBy : undefined

    const sortPart = sort ? `record[property.name] ${sort}` : `record.${RUSHDB_KEY_ID}`

    const paginationParams = pagination(searchQuery.skip, searchQuery.limit)

    const { sortedQueryParts, parsedWhere } = buildQuery(searchQuery)
    const labelPart = singleLabelPart(searchQuery?.labels)

    const queryClauses = buildQueryClause({
      queryParts: sortedQueryParts,
      labelClause: buildLabelsClause(searchQuery?.labels)
    })

    const wherePart = parsedWhere.where ? `WHERE ${parsedWhere.where}` : ''
    const whereClause = `WITH property, ${parsedWhere.nodeAliases.join(', ')} ${wherePart}`.trim()

    const queryBuilder = new QueryBuilder()

    queryBuilder.append(
      `MATCH (record:${RUSHDB_LABEL_RECORD}${labelPart} { ${projectIdInline()} })<-[value:${RUSHDB_RELATION_VALUE}]-(property:${RUSHDB_LABEL_PROPERTY} { id: $id })`
    )

    if (queryClauses.length > 0) {
      queryBuilder
        .append(queryClauses.join(`\n`))
        .append(`AND record[property.name] IS NOT NULL AND property.type <> 'vector'`)
        .append(
          searchQuery.query ?
            `AND any(value IN record[property.name] WHERE value  =~ "(?i).*${searchQuery.query}.*")`
          : ''
        )
        .append(whereClause)
    } else {
      queryBuilder
        .append(`WHERE record[property.name] IS NOT NULL AND property.type <> 'vector'`)
        .append(
          searchQuery.query ?
            `AND any(value IN record[property.name] WHERE value  =~ "(?i).*${searchQuery.query}.*")`
          : ''
        )
    }

    queryBuilder
      .append(`WITH record[property.name] AS propValue, property.type AS propType`)
      .append(`ORDER BY ${sortPart}`)
      .append(
        `WITH apoc.coll.toSet(apoc.coll.flatten(collect(DISTINCT propValue))) AS values, propType as type`
      )
      .append(`UNWIND values AS v`)
      .append(
        `WITH values, type, min(CASE type WHEN 'datetime' THEN datetime(v) ELSE toFloatOrNull(v) END) AS minValue, max(CASE type WHEN 'datetime' THEN datetime(v) ELSE toFloatOrNull(v) END) AS maxValue`
      )
      .append(`RETURN {`)
      .append(`values: values[${paginationParams.skip}..${paginationParams.skip + paginationParams.limit}],`)
      .append(`min: CASE type WHEN 'datetime' THEN toString(minValue) ELSE minValue END,`)
      .append(`max: CASE type WHEN 'datetime' THEN toString(maxValue) ELSE maxValue END,`)
      .append(`type: type`)
      .append(`} AS result`)

    return queryBuilder.getQuery()
  }

  cleanUpAfterProcessing() {
    const queryBuilder = new QueryBuilder()

    queryBuilder.append(
      `MATCH (lost:${RUSHDB_LABEL_PROPERTY} { ${projectIdInline()} }) WHERE NOT (lost)--() DELETE lost`
    )

    return queryBuilder.getQuery()
  }
}
