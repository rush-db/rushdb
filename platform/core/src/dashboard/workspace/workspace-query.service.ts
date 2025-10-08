import { Injectable } from '@nestjs/common'

import {
  RUSHDB_LABEL_PROJECT,
  RUSHDB_LABEL_USER,
  RUSHDB_LABEL_WORKSPACE,
  RUSHDB_RELATION_CONTAINS,
  RUSHDB_RELATION_HAS_ACCESS,
  RUSHDB_RELATION_MEMBER_OF
} from '@/dashboard/common/constants'
import { USER_ROLE_EDITOR, USER_ROLE_OWNER } from '@/dashboard/user/interfaces/user.constants'
import { QueryBuilder } from '@/database/QueryBuilder'

@Injectable()
export class WorkspaceQueryService {
  getWorkspaceUserListQuery() {
    const queryBuilder = new QueryBuilder()

    queryBuilder.append(`MATCH (w:${RUSHDB_LABEL_WORKSPACE} {id: $workspaceId})`)
    queryBuilder.append(`MATCH (u:${RUSHDB_LABEL_USER})-[rel:${RUSHDB_RELATION_MEMBER_OF}]->(w)`)
    queryBuilder.append(`RETURN DISTINCT u.id AS id, u.login AS login, rel.role AS role`)

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
        `OPTIONAL MATCH (u)-[:${RUSHDB_RELATION_MEMBER_OF} { role: '${USER_ROLE_OWNER}' }]->(w1:${RUSHDB_LABEL_WORKSPACE})`
      )
      .append(`WHERE w1.id <> $workspaceId`)
      .append(
        `OPTIONAL MATCH (u)-[:${RUSHDB_RELATION_MEMBER_OF} { role: '${USER_ROLE_EDITOR}' }]->(w2:${RUSHDB_LABEL_WORKSPACE})`
      )
      .append(`WHERE w2.id <> $workspaceId`)
      .append(`RETURN count(DISTINCT w1) AS ownerOther, count(DISTINCT w2) AS developerOther`)

    return queryBuilder.getQuery()
  }

  getRemoveWorkspaceRelationQuery(): string {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(
        `MATCH (u:${RUSHDB_LABEL_USER} {id: $userId})-[r:${RUSHDB_RELATION_MEMBER_OF}]->(w:${RUSHDB_LABEL_WORKSPACE} {id: $workspaceId})`
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
      .append(`WHERE (:${RUSHDB_LABEL_WORKSPACE} {id: $workspaceId})-[:${RUSHDB_RELATION_CONTAINS}]->(p)`)
      .append(`DELETE r`)

    return queryBuilder.getQuery()
  }

  getUserWorkspaceRoleQuery(): string {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(
        `MATCH (u:${RUSHDB_LABEL_USER} { login: $login })-[rel:${RUSHDB_RELATION_MEMBER_OF}]->(w:${RUSHDB_LABEL_WORKSPACE} { id: $workspaceId })`
      )
      .append(`RETURN rel.role AS role`)

    return queryBuilder.getQuery()
  }

  getPendingInvitesQuery(): string {
    const queryBuilder = new QueryBuilder()

    return queryBuilder
      .append(`MATCH (w:${RUSHDB_LABEL_WORKSPACE} {id: $workspaceId})`)
      .append(`RETURN w.pendingInvites AS invites`)
      .getQuery()
  }

  setPendingInvitesQuery(): string {
    const queryBuilder = new QueryBuilder()

    return queryBuilder
      .append(`MATCH (w:${RUSHDB_LABEL_WORKSPACE} {id: $workspaceId})`)
      .append(`SET w.pendingInvites = $invites`)
      .getQuery()
  }

  attachUserToWorkspaceQuery(): string {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(
        `MATCH (u:${RUSHDB_LABEL_USER} { id: $userId }), (w:${RUSHDB_LABEL_WORKSPACE} { id: $workspaceId })`
      )
      .append(`MERGE (u)-[r:${RUSHDB_RELATION_MEMBER_OF}]->(w)`)
      .append(`ON CREATE SET r.since = $since, r.role = $role`)
      .append(`ON MATCH SET r.since = $since, r.role = $role`)

    return queryBuilder.getQuery()
  }
}
