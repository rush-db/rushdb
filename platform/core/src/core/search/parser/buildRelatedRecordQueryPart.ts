import { RUSHDB_KEY_ID, RUSHDB_LABEL_RECORD, RUSHDB_RELATION_DEFAULT } from '@/core/common/constants'
import { RELATION_DIRECTION_IN, RELATION_DIRECTION_OUT } from '@/core/entity/entity.constants'
import { TRelationDirection } from '@/core/entity/entity.types'
import { projectIdInline } from '@/core/search/parser/projectIdInline'

export const buildRelationPart = (relation?: { type?: string; direction?: TRelationDirection }) => {
  if (relation) {
    const { type, direction } = relation
    let relationPart = type ? `-[rel:${type}]-` : `-[rel:${RUSHDB_RELATION_DEFAULT}]-`

    if (direction === RELATION_DIRECTION_IN) {
      relationPart = '<' + relationPart
    }

    if (direction === RELATION_DIRECTION_OUT) {
      relationPart = relationPart + '>'
    }

    return relationPart
  }
  return '-[rel]-'
}

export const buildRelatedQueryPart = (
  id?: string,
  relation?: {
    type?: string
    direction?: TRelationDirection
  }
) => {
  const relationPart = buildRelationPart(relation)

  return id
    ? `(source:${RUSHDB_LABEL_RECORD} { ${projectIdInline()}, ${RUSHDB_KEY_ID}: "${id}" })${relationPart}`
    : ''
}
