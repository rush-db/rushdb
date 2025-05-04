import { Injectable } from '@nestjs/common'

import { QueryBuilder } from '@/common/QueryBuilder'
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
}
