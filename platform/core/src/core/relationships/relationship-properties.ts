import { BadRequestException } from '@nestjs/common'

import { isArray } from '@/common/utils/isArray'
import { isObject } from '@/common/utils/isObject'
import {
  RUSHDB_KEY_ID,
  RUSHDB_KEY_PROJECT_ID,
  RUSHDB_KEY_PROPERTIES_META,
  RUSHDB_KEY_LABEL
} from '@/core/common/constants'

export type RelationshipProperties = Record<string, unknown>

const RESERVED_RELATIONSHIP_PROPERTY_NAMES = new Set([
  RUSHDB_KEY_ID,
  RUSHDB_KEY_PROJECT_ID,
  RUSHDB_KEY_PROPERTIES_META,
  RUSHDB_KEY_LABEL,
  'sourceId',
  'targetId',
  'sourceLabel',
  'targetLabel',
  'type',
  'direction',
  'id',
  'elementId',
  'projectId'
])

const isSupportedScalar = (value: unknown) =>
  typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean' || value === null

const validateRelationshipPropertyName = (name: string) => {
  if (!name || RESERVED_RELATIONSHIP_PROPERTY_NAMES.has(name) || name.startsWith('__RUSHDB__')) {
    throw new BadRequestException(`"${name}" is a reserved relationship property name`)
  }
}

const validateRelationshipPropertyValue = (name: string, value: unknown) => {
  if (isSupportedScalar(value)) {
    return
  }

  if (isArray(value)) {
    if (value.every(isSupportedScalar)) {
      const nonNullTypes = value.filter((item) => item !== null).map((item) => typeof item)
      const [firstType] = nonNullTypes
      if (!firstType || nonNullTypes.every((type) => type === firstType)) {
        return
      }
    }
  }

  throw new BadRequestException(
    `Unsupported relationship property "${name}". Use primitive values or homogeneous primitive arrays.`
  )
}

export const normalizeRelationshipProperties = (
  properties?: RelationshipProperties
): RelationshipProperties => {
  if (!properties) {
    return {}
  }

  if (!isObject(properties) || isArray(properties)) {
    throw new BadRequestException('Relationship properties must be an object')
  }

  for (const [name, value] of Object.entries(properties)) {
    validateRelationshipPropertyName(name)
    validateRelationshipPropertyValue(name, value)
  }

  return properties
}

export const RESERVED_RELATIONSHIP_PROPERTY_KEYS = [
  ...RESERVED_RELATIONSHIP_PROPERTY_NAMES,
  '__projectId',
  '__propKey'
]
