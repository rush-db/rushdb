import { Injectable } from '@nestjs/common'

import { RUSHDB_LABEL_PROPERTY, RUSHDB_KEY_PROJECT_ID, RUSHDB_LABEL_RECORD } from '@/core/common/constants'
import { projectIdInline } from '@/core/search/parser/projectIdInline'
import {
  RUSHDB_LABEL_PROJECT,
  RUSHDB_LABEL_TOKEN,
  RUSHDB_LABEL_USER,
  RUSHDB_LABEL_WORKSPACE,
  RUSHDB_RELATION_CONTAINS,
  RUSHDB_RELATION_HAS_ACCESS
} from '@/dashboard/common/constants'
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

  removeProjectNodeQuery() {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`MATCH (project:${RUSHDB_LABEL_PROJECT} { id: $projectId })`)
      .append(`OPTIONAL MATCH (project)<-[:${RUSHDB_RELATION_HAS_ACCESS}]-(token:${RUSHDB_LABEL_TOKEN})`)
      .append(`DETACH DELETE project, token`)

    return queryBuilder.getQuery()
  }

  projectRelatedUserIdsQuery() {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(
        `MATCH (project:${RUSHDB_LABEL_PROJECT} {id: $projectId})<-[:${RUSHDB_RELATION_HAS_ACCESS} { role: $role }]-(relatedUser:${RUSHDB_LABEL_USER})`
      )
      .append(`WITH relatedUser, collect(DISTINCT relatedUser.id) as usersId`)
      .append(`RETURN usersId`)

    return queryBuilder.getQuery()
  }

  grantUserAccessQuery() {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`MATCH (user:${RUSHDB_LABEL_USER} { id: $userId })`)
      .append(`MATCH (project:${RUSHDB_LABEL_PROJECT} { id: $projectId })`)
      .append(
        `CREATE (project)<-[newRel:${RUSHDB_RELATION_HAS_ACCESS} { since: $since, role: $role }]-(user)`
      )

    return queryBuilder.getQuery()
  }

  revokeUserAccessQuery() {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`MATCH (user:${RUSHDB_LABEL_USER} { id: $userId })`)
      .append(
        `OPTIONAL MATCH (project:${RUSHDB_LABEL_PROJECT} { id: $projectId })<-[oldRel:${RUSHDB_RELATION_HAS_ACCESS}]-(user)`
      )
      .append(`DELETE oldRel`)

    return queryBuilder.getQuery()
  }

  getProjectsByWorkspaceId() {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(
        `MATCH (o:${RUSHDB_LABEL_WORKSPACE} { id: $id })-[:${RUSHDB_RELATION_CONTAINS}]->(p:${RUSHDB_LABEL_PROJECT})`
      )
      .append(`WHERE p.deleted IS null`)
      .append(`RETURN collect(DISTINCT p) as projects`)

    return queryBuilder.getQuery()
  }

  getUserRelatedProjects() {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(
        `MATCH (w:${RUSHDB_LABEL_WORKSPACE} { id: $id })-[:${RUSHDB_RELATION_CONTAINS}]->(p:${RUSHDB_LABEL_PROJECT})`
      )
      .append(`WHERE p.deleted IS null`)
      .append(`MATCH (u:${RUSHDB_LABEL_USER} { id: $userId })-[:${RUSHDB_RELATION_HAS_ACCESS}]->(p)`)
      .append(`RETURN collect(DISTINCT p) as projects`)

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
      .append(`   RETURN count(r) AS entities, round(avg(size(keys(r))), 2) AS avg`)
      .append(`}`)
      .append(`RETURN properties, entities, avg`)

    return queryBuilder.getQuery()
  }

  getAttachUserToProjectQuery(): string {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(
        `MATCH (u:${RUSHDB_LABEL_USER} { id: $userId }), (p:${RUSHDB_LABEL_PROJECT} { id: $projectId })`
      )
      .append(`MERGE (u)-[r:${RUSHDB_RELATION_HAS_ACCESS}]->(p)`)
      .append(`ON CREATE SET r.since = $since, r.role = $role`)
      .append(`ON MATCH  SET r.since = $since, r.role = $role`)

    return queryBuilder.getQuery()
  }
}
