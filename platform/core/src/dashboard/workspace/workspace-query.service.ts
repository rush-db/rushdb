import { Injectable } from '@nestjs/common'

import { QueryBuilder } from '@/common/QueryBuilder'
import {
  RUSHDB_LABEL_PROJECT,
  RUSHDB_LABEL_USER,
  RUSHDB_LABEL_WORKSPACE,
  RUSHDB_RELATION_CONTAINS,
  RUSHDB_RELATION_HAS_ACCESS
} from '@/dashboard/common/constants'
import { USER_ROLE_EDITOR, USER_ROLE_OWNER } from '@/dashboard/user/interfaces/user.constants'

@Injectable()
export class WorkspaceQueryService {
  getWorkspaceUserListQuery() {
    const queryBuilder = new QueryBuilder()

    queryBuilder.append(
      `MATCH (w:${RUSHDB_LABEL_WORKSPACE} {id: $workspaceId})-[:${RUSHDB_RELATION_CONTAINS}]->(p:${RUSHDB_LABEL_PROJECT})`
    )
    queryBuilder.append(`WHERE p.deleted IS NULL`)
    queryBuilder.append(
      `MATCH (u:${RUSHDB_LABEL_USER})-[:${RUSHDB_RELATION_HAS_ACCESS} { role: $role }]->(p)`
    )
    queryBuilder.append(`RETURN DISTINCT u.id AS id, u.login AS login`)

    return queryBuilder.getQuery()
  }

  getWorkspaceAccessListQuery() {
    const queryBuilder = new QueryBuilder()

    queryBuilder.append(
      `MATCH (w:${RUSHDB_LABEL_WORKSPACE} {id: $workspaceId})-[:${RUSHDB_RELATION_CONTAINS}]->(p:${RUSHDB_LABEL_PROJECT})`
    )
    queryBuilder.append(`WHERE p.deleted IS NULL`)
    queryBuilder.append(
      `OPTIONAL MATCH (u:${RUSHDB_LABEL_USER})-[r:${RUSHDB_RELATION_HAS_ACCESS} { role: $role }]->(p)`
    )
    queryBuilder.append(
      `WITH p.id AS projectId, collect(DISTINCT CASE WHEN u.id IS NOT NULL THEN u.id END) AS userIds`
    )
    queryBuilder.append(`RETURN projectId, userIds`)

    return queryBuilder.getQuery()
  }

  getUserRoleCountsOutsideWorkspaceQuery(): string {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(`MATCH (u:${RUSHDB_LABEL_USER} {id: $userId})`)
      .append(
        `OPTIONAL MATCH (u)-[:${RUSHDB_RELATION_HAS_ACCESS} { role: '${USER_ROLE_OWNER}' }]->(w1:${RUSHDB_LABEL_WORKSPACE})`
      )
      .append(`WHERE w1.id <> $workspaceId`)
      .append(
        `OPTIONAL MATCH (u)-[:${RUSHDB_RELATION_HAS_ACCESS} { role: '${USER_ROLE_EDITOR}' }]->(w2:${RUSHDB_LABEL_WORKSPACE})`
      )
      .append(`WHERE w2.id <> $workspaceId`)
      .append(`RETURN count(DISTINCT w1) AS ownerOther, count(DISTINCT w2) AS developerOther`)

    return queryBuilder.getQuery()
  }

  getRemoveWorkspaceRelationQuery(): string {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(
        `MATCH (u:${RUSHDB_LABEL_USER} {id: $userId})-[r:${RUSHDB_RELATION_HAS_ACCESS}]->(w:${RUSHDB_LABEL_WORKSPACE} {id: $workspaceId})`
      )
      .append(`DELETE r`)

    return queryBuilder.getQuery()
  }

  getRemoveProjectRelationsQuery(): string {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(
        `MATCH (u:${RUSHDB_LABEL_USER} {id: $userId})-[r:${RUSHDB_RELATION_HAS_ACCESS}]->(p:${RUSHDB_LABEL_PROJECT})`
      )
      .append(`WHERE (p)-[:${RUSHDB_RELATION_CONTAINS}]->(:${RUSHDB_LABEL_WORKSPACE} {id: $workspaceId})`)
      .append(`DELETE r`)

    return queryBuilder.getQuery()
  }
}
