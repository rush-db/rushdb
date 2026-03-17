import { Injectable } from '@nestjs/common'

import { RUSHDB_LABEL_PROPERTY, RUSHDB_KEY_PROJECT_ID, RUSHDB_LABEL_RECORD } from '@/core/common/constants'
import { projectIdInline } from '@/core/search/parser/projectIdInline'
import { QueryBuilder } from '@/database/QueryBuilder'

@Injectable()
export class ProjectQueryService {
  removeProjectQuery() {
    const batchConfig = `{ batchSize: 5000, concurrency: 10, parallel: true, params: { projectId: $projectId }, batchMode: "BATCH", retries: 5 }`
    // @FYI: deletion process is intentionally separated to ensure that every distinctively deletable
    // entity (record, property, project, token, meta) is deleted properly and there is no orphans
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`CALL apoc.periodic.iterate(`)
      .append(`"MATCH (record:${RUSHDB_LABEL_RECORD} { ${projectIdInline()} }) RETURN record",`)
      .append(`"WITH record DETACH DELETE record",`)
      .append(batchConfig)
      .append(`) YIELD batch as b1, operations as o1`)
      //
      .append(`CALL apoc.periodic.iterate(`)
      .append(`"MATCH (property:${RUSHDB_LABEL_PROPERTY} { ${projectIdInline()} }) RETURN property",`)
      .append(`"WITH property DETACH DELETE property",`)
      .append(batchConfig)
      .append(`) YIELD batch as b2, operations as o2`)
      .append(`RETURN b1,b2,o1,o2`)

    return queryBuilder.getQuery()
  }

  removeRemoteDbDataQuery() {
    const batchConfig = `{ batchSize: 5000, concurrency: 10, parallel: true, params: { projectId: $projectId }, batchMode: "BATCH", retries: 5 }`
    // @FYI: deletion process is intentionally separated to ensure that every distinctively deletable
    // entity (record, property, project, token, meta) is deleted properly and there is no orphans
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`CALL apoc.periodic.iterate(`)
      .append(`"MATCH (record:${RUSHDB_LABEL_RECORD} { ${projectIdInline()} }) RETURN record",`)
      .append(`"WITH record DETACH DELETE record",`)
      .append(batchConfig)
      .append(`) YIELD batch as b1, operations as o1`)
      //
      .append(`CALL apoc.periodic.iterate(`)
      .append(`"MATCH (property:${RUSHDB_LABEL_PROPERTY} { ${projectIdInline()} }) RETURN property",`)
      .append(`"WITH property DETACH DELETE property",`)
      .append(batchConfig)
      .append(`) YIELD batch as b2, operations as o2`)
      .append(`RETURN b1,b2,o1,o2`)

    return queryBuilder.getQuery()
  }

  getProjectStatsById() {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`CALL {`)
      .append(`   MATCH (p:${RUSHDB_LABEL_PROPERTY} {projectId: $id})`)
      .append(`   RETURN count(p) AS properties`)
      .append(`}`)
      .append(`CALL {`)
      .append(`   MATCH (r:${RUSHDB_LABEL_RECORD} {${RUSHDB_KEY_PROJECT_ID}: $id})`)
      .append(`   RETURN count(r) AS entities`)
      .append(`}`)
      .append(`RETURN properties, entities`)

    return queryBuilder.getQuery()
  }
}
