import { Injectable } from '@nestjs/common'

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
export class TokenQueryService {
  traverseTokenData() {
    const queryBuilder = new QueryBuilder()

    queryBuilder
      .append(
        `MATCH (token:${RUSHDB_LABEL_TOKEN} { id: $tokenId })-[relation]->(project:${RUSHDB_LABEL_PROJECT})--(workspace:${RUSHDB_LABEL_WORKSPACE})`
      )
      .append(`RETURN token, project, workspace, relation.level as level`)

    return queryBuilder.build()
  }
  validateIntegrity({
    userId,
    workspaceId,
    projectId
  }: {
    userId: string
    workspaceId: string
    projectId?: string
  }) {
    const queryBuilder = new QueryBuilder()

    queryBuilder.append(
      `MATCH (workspace:${RUSHDB_LABEL_WORKSPACE} { id: "${workspaceId}" })--(user:${RUSHDB_LABEL_USER} { id: "${userId}" })`
    )

    if (projectId) {
      queryBuilder.append(
        `OPTIONAL MATCH (user)-[relation:${RUSHDB_RELATION_HAS_ACCESS}]->(project:${RUSHDB_LABEL_PROJECT} { id: "${projectId}" })--(workspace)`
      )
      queryBuilder.append(`RETURN project, workspace, relation.level as level`)
    } else {
      queryBuilder.append(`RETURN workspace`)
    }

    return queryBuilder.build()
  }
}
