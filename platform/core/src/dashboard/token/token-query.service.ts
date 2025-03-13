import { Injectable } from '@nestjs/common'

import { QueryBuilder } from '@/common/QueryBuilder'
import {
  RUSHDB_LABEL_PROJECT,
  RUSHDB_LABEL_TOKEN,
  RUSHDB_LABEL_WORKSPACE
} from '@/dashboard/common/constants'

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
}
