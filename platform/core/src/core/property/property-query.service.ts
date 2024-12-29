import { Injectable } from '@nestjs/common'

import { toBoolean } from '@/common/utils/toBolean'
import {
  RUSHDB_KEY_ID,
  RUSHDB_KEY_PROPERTIES_META,
  RUSHDB_LABEL_PROPERTY,
  RUSHDB_LABEL_RECORD,
  RUSHDB_RELATION_VALUE
} from '@/core/common/constants'
import { SearchDto } from '@/core/search/dto/search.dto'
import { buildPagination } from '@/core/search/parser/buildQuery'
import { TSearchSortDirection } from '@/core/search/search.types'

@Injectable()
export class PropertyQueryService {
  affectedRecordsMatchingPart(entityIds?: string[]) {
    return entityIds?.length
      ? `
                UNWIND $entityIds as entityId
                CALL {
                   WITH entityId
                   MATCH (record:${RUSHDB_LABEL_RECORD} { ${RUSHDB_KEY_ID}: entityId })<-[value:${RUSHDB_RELATION_VALUE}]-(property:${RUSHDB_LABEL_PROPERTY} {id: $target})
                   RETURN DISTINCT record, value, property
                }
                WITH *
            `
      : `
                MATCH (record:${RUSHDB_LABEL_RECORD}),
                (record)<-[value:${RUSHDB_RELATION_VALUE}]-(property:${RUSHDB_LABEL_PROPERTY} { id: $target })
            `
  }
  deleteField(entityIds?: string[]) {
    return `
            ${this.affectedRecordsMatchingPart(entityIds)}
                
            DELETE value
                
            WITH *, collect(property.name) AS propertyKeyToDelete, 
                apoc.convert.fromJsonMap(record.${RUSHDB_KEY_PROPERTIES_META}) AS recordPropertiesMetaMap
            
            CALL apoc.create.removeProperties(record, propertyKeyToDelete) YIELD node as _result0
                
            WITH *, 
                apoc.map.removeKey(recordPropertiesMetaMap, property.name) as updatedRecordPropertiesMetaMap
            
            SET record.${RUSHDB_KEY_PROPERTIES_META} = apoc.convert.toJson(updatedRecordPropertiesMetaMap)
        `
  }

  updateField(entityIds?: string[]) {
    return `
            ${this.affectedRecordsMatchingPart(entityIds)}
            DELETE value
            
            WITH *, apoc.convert.toList($newValue) as valueRaw
            WITH *, range(0, size(valueRaw)-1) AS valueIndexesList
            UNWIND valueIndexesList AS index
            
            WITH property, record, index, CASE property.type WHEN "datetime" THEN datetime(valueRaw[index]) ELSE valueRaw[index] END as value,
                apoc.map.setEntry(record, property.name, valueRaw) AS resultMeta

            SET record += resultMeta

            MERGE (property)-[rel:${RUSHDB_RELATION_VALUE}]->(record)
        `
  }

  updateFieldNode() {
    return `
            MATCH (record:${RUSHDB_LABEL_RECORD})<-[value:${RUSHDB_RELATION_VALUE}]-(property:${RUSHDB_LABEL_PROPERTY} {id: $target})
            WITH *, CASE WHEN $newName IS NOT NULL THEN $newName ELSE property.name END as nameToSet,
                CASE WHEN $newType IS NOT NULL THEN $newType ELSE property.type END as typeToSet
            
            WITH *, collect(property.name) AS propertyKeyToDelete, 
                apoc.convert.fromJsonMap(record.${RUSHDB_KEY_PROPERTIES_META}) AS recordPropertiesMetaMap
            CALL apoc.create.removeProperties(record, propertyKeyToDelete) YIELD node as _result0
            
            WITH *, 
                apoc.map.removeKey(recordPropertiesMetaMap, property.name) as cleanRecordPropertiesMetaMap
            WITH *, 
                apoc.map.setEntry(cleanRecordPropertiesMetaMap, nameToSet, typeToSet) AS updatedMeta
            WITH *, 
                apoc.convert.toList(toString(value.value)) as valueRaw
            WITH *, 
                range(0, size(valueRaw)-1) AS valueIndexesList

            SET record.${RUSHDB_KEY_PROPERTIES_META} = apoc.convert.toJson(updatedMeta)
            
            WITH *
            UNWIND valueIndexesList AS index
            
            WITH nameToSet, typeToSet, property, record, index, CASE property.type WHEN "datetime" THEN datetime(valueRaw[index]) ELSE valueRaw[index] END as value,
                apoc.map.setEntry(record, nameToSet, valueRaw) AS resultMeta

            SET record += resultMeta

            MERGE (property)-[rel:${RUSHDB_RELATION_VALUE}]->(record)
            
            SET property.name = nameToSet
            SET property.type = typeToSet
        `
  }

  mergeFieldMeta() {
    return `
            MATCH (record:${RUSHDB_LABEL_RECORD})<-[value:${RUSHDB_RELATION_VALUE}]-(property:${RUSHDB_LABEL_PROPERTY} { id: $target })
            WITH *, CASE WHEN $newType IS NOT NULL THEN $newType ELSE property.type END as typeToSet

            OPTIONAL MATCH (conflictProperty:${RUSHDB_LABEL_PROPERTY} { id: $conflictNodeId })
            OPTIONAL MATCH (conflictProperty)-[existingValue:${RUSHDB_RELATION_VALUE}]->(record)
            
            WITH *, collect(value.value) as valuesToAttach, collect(existingValue.value) as sourceValues
            
            DELETE existingValue
            
            WITH *
            
            CALL {
                WITH *
                UNWIND valuesToAttach as valueToAttach
                RETURN CASE WHEN (typeToSet = "string" AND property.type <> "string") 
                    THEN toString(valueToAttach) 
                    ELSE valueToAttach
                END as rawValue
            }
            
            WITH conflictProperty, typeToSet, property, record, sourceValues, collect(rawValue) as rawValues
            
            CALL {
                WITH *
                UNWIND (CASE sourceValues WHEN [] THEN [null] ELSE sourceValues END) as rawSourceValue
                RETURN rawSourceValue
            }
            
            WITH conflictProperty, typeToSet, property, record, rawValues, collect(rawSourceValue) as sourceRawValues
            WITH conflictProperty, typeToSet, property, record, sourceRawValues + rawValues as valueRaw

            WITH conflictProperty,typeToSet, property, record, valueRaw,
                range(0, size(valueRaw)-1) AS valueIndexesList

            UNWIND valueIndexesList AS index

            WITH conflictProperty, typeToSet, property, record, index, valueRaw[index] as value,
                apoc.map.setEntry(record, conflictProperty.name, valueRaw) AS resultMeta

            SET record += resultMeta
            
            WITH *, apoc.convert.fromJsonMap(record.${RUSHDB_KEY_PROPERTIES_META}) AS recordPropertiesMetaMap
            WITH *, apoc.map.setEntry(recordPropertiesMetaMap, conflictProperty.name, typeToSet) AS updatedMeta
            
            SET record.${RUSHDB_KEY_PROPERTIES_META} = apoc.convert.toJson(updatedMeta)

            MERGE (conflictProperty)-[rel:${RUSHDB_RELATION_VALUE}]->(record)
    
        `
  }

  getProjectProperties() {
    return `
            MATCH (property:${RUSHDB_LABEL_PROPERTY} { projectId: $id } )
            RETURN collect(DISTINCT property { .id, .metadata, .name, .type, .projectId }) as properties
        `
  }

  getPropertyValues({
    sort,
    paginationParams
  }: {
    sort?: TSearchSortDirection
    paginationParams?: Pick<SearchDto, 'skip' | 'limit'>
  }) {
    const pagination = buildPagination(
      toBoolean(paginationParams) ? paginationParams : { skip: 0, limit: 50 }
    )

    const sortPart = sort ? `record[property.name] ${sort}` : `record.${RUSHDB_KEY_ID}`

    return `MATCH (record:${RUSHDB_LABEL_RECORD})<-[value:${RUSHDB_RELATION_VALUE}]-(property:${RUSHDB_LABEL_PROPERTY} { id: $id })
        WHERE record[property.name] IS NOT NULL
        ORDER BY ${sortPart} ${pagination}
        RETURN {
            values: collect(DISTINCT record[property.name]),
            min: toFloatOrNull(min(toFloatOrNull(record[property.name]))),
            max: toFloatOrNull(max(toFloatOrNull(record[property.name]))),
            type: collect(DISTINCT property.type)[0]
        } AS result`
  }

  cleanUpAfterProcessing() {
    return `
            MATCH (lost:${RUSHDB_LABEL_PROPERTY} { projectId: $projectId }) WHERE NOT (lost)--() DELETE lost
        `
  }
}
