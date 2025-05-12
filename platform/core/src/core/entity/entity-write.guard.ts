import { CanActivate, ExecutionContext, Injectable, BadRequestException } from '@nestjs/common'

import { isArray } from '@/common/utils/isArray'
import {
  RUSHDB_KEY_ID,
  RUSHDB_KEY_ID_ALIAS,
  RUSHDB_KEY_LABEL,
  RUSHDB_KEY_LABEL_ALIAS,
  RUSHDB_KEY_PROJECT_ID,
  RUSHDB_KEY_PROJECT_ID_ALIAS,
  RUSHDB_KEY_PROPERTIES_META,
  RUSHDB_KEY_PROPERTIES_META_ALIAS
} from '@/core/common/constants'
import {
  ALIAS_CLAUSE_OPERATOR,
  comparisonOperators,
  datetimeOperators,
  ID_CLAUSE_OPERATOR,
  logicalOperators,
  RELATION_CLAUSE_OPERATOR,
  vectorOperators
} from '@/core/search/search.constants'

const publiclyDisallowedKeys = [
  ...logicalOperators,
  ...comparisonOperators,
  ...datetimeOperators,
  ...vectorOperators,
  RUSHDB_KEY_ID_ALIAS,
  RUSHDB_KEY_PROJECT_ID_ALIAS,
  RUSHDB_KEY_PROPERTIES_META_ALIAS,
  RUSHDB_KEY_LABEL_ALIAS,
  RELATION_CLAUSE_OPERATOR,
  ID_CLAUSE_OPERATOR,
  ALIAS_CLAUSE_OPERATOR
]
// @FYI: intentionally holding internal aliases hidden (even for error when matched for it's low probability)
const disallowedKeys = [
  ...publiclyDisallowedKeys,
  RUSHDB_KEY_ID,
  RUSHDB_KEY_PROJECT_ID,
  RUSHDB_KEY_PROPERTIES_META,
  RUSHDB_KEY_LABEL
]

@Injectable()
export class EntityWriteGuard implements CanActivate {
  canActivate(context: ExecutionContext): boolean {
    const request = context.switchToHttp().getRequest()
    const body = request.body

    if (this.containsDisallowedKeys(body)) {
      throw new BadRequestException(
        `Request body contains disallowed keys: ${JSON.stringify(publiclyDisallowedKeys)}`
      )
    }

    return true
  }

  private containsDisallowedKeys(obj: any): boolean {
    if (obj && typeof obj === 'object') {
      if (isArray(obj)) {
        for (const item of obj) {
          if (this.containsDisallowedKeys(item)) {
            return true
          }
        }
      } else {
        for (const key in obj) {
          if (disallowedKeys.includes(key)) {
            return true
          }
          if (this.containsDisallowedKeys(obj[key])) {
            return true
          }
        }
      }
    }
    return false
  }
}
