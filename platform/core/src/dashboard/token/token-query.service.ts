import { Injectable } from '@nestjs/common'

import {
  RUSHDB_LABEL_PROJECT,
  RUSHDB_LABEL_TOKEN,
  RUSHDB_LABEL_WORKSPACE
} from '@/dashboard/common/constants'

@Injectable()
export class TokenQueryService {
  traverseTokenData() {
    return `
            MATCH (token:${RUSHDB_LABEL_TOKEN} { id: $tokenId })-[relation]->(project:${RUSHDB_LABEL_PROJECT})--(workspace:${RUSHDB_LABEL_WORKSPACE})
            RETURN token, project, workspace, relation.level as level
        `
  }
}
