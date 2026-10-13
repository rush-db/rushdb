import { IMPORT_ERROR_CODES } from './import-run.types'

import type { ImportLinkSpec } from './import-run.types'

const LABEL_PATTERN = /^[A-Za-z][A-Za-z0-9_]{0,99}$/

function isValidPropertyKey(raw: string): boolean {
  if (raw.length === 0 || raw.length > 255) {
    return false
  }
  for (let i = 0; i < raw.length; i++) {
    const code = raw.charCodeAt(i)
    if (code <= 0x1f || code === 0x60) {
      return false
    }
  }
  return true
}

export class ImportValidationError extends Error {
  constructor(
    public readonly code: string,
    message: string
  ) {
    super(message)
  }
}

export function validateLinkSpec(spec: unknown): asserts spec is ImportLinkSpec {
  if (!spec || typeof spec !== 'object') {
    throw new ImportValidationError(
      IMPORT_ERROR_CODES.LINK_SPEC_INVALID,
      'linkSpec is required for links files'
    )
  }

  const candidate = spec as Partial<ImportLinkSpec>

  if (candidate.version !== 1 || candidate.role !== 'links') {
    throw new ImportValidationError(
      IMPORT_ERROR_CODES.LINK_SPEC_INVALID,
      'linkSpec.version must be 1 and role "links"'
    )
  }

  if (!Array.isArray(candidate.endpoints) || candidate.endpoints.length !== 2) {
    throw new ImportValidationError(
      IMPORT_ERROR_CODES.LINK_SPEC_INVALID,
      'linkSpec requires exactly two endpoints (source and target)'
    )
  }

  const directions = candidate.endpoints.map((e) => e?.direction)
  if (!directions.includes('source') || !directions.includes('target')) {
    throw new ImportValidationError(
      IMPORT_ERROR_CODES.LINK_SPEC_INVALID,
      'linkSpec endpoints must include exactly one source and one target'
    )
  }

  for (const endpoint of candidate.endpoints) {
    if (!endpoint.column || !isValidPropertyKey(endpoint.column)) {
      throw new ImportValidationError(
        IMPORT_ERROR_CODES.LINK_SPEC_INVALID,
        `invalid endpoint column: ${endpoint.column}`
      )
    }
    if (!endpoint.label || !LABEL_PATTERN.test(endpoint.label)) {
      throw new ImportValidationError(
        IMPORT_ERROR_CODES.LINK_SPEC_INVALID,
        `invalid endpoint label: ${endpoint.label}`
      )
    }
    if (!endpoint.keyProperty || !isValidPropertyKey(endpoint.keyProperty)) {
      throw new ImportValidationError(
        IMPORT_ERROR_CODES.LINK_SPEC_INVALID,
        `invalid endpoint keyProperty: ${endpoint.keyProperty}`
      )
    }
  }

  if (
    candidate.endpoints[0].column.toLowerCase() === candidate.endpoints[1].column.toLowerCase() &&
    candidate.endpoints[0].label === candidate.endpoints[1].label
  ) {
    throw new ImportValidationError(
      IMPORT_ERROR_CODES.LINK_SPEC_INVALID,
      'endpoints must bind distinct columns'
    )
  }

  if (!candidate.relationshipType || !isValidPropertyKey(candidate.relationshipType)) {
    throw new ImportValidationError(IMPORT_ERROR_CODES.LINK_SPEC_INVALID, 'relationshipType is required')
  }

  if (candidate.propertyColumns) {
    for (const [column, property] of Object.entries(candidate.propertyColumns)) {
      if (!isValidPropertyKey(column) || !isValidPropertyKey(property)) {
        throw new ImportValidationError(
          IMPORT_ERROR_CODES.LINK_COLUMNS_UNMAPPED,
          `invalid property column mapping: ${column}`
        )
      }
    }
  }
}

/**
 * Ensures every non-endpoint column is either explicitly mapped as a relationship
 * property or listed in ignoredColumns. Silent dropping is forbidden.
 */
export function validateLinkColumns(spec: ImportLinkSpec, columns: string[]): void {
  const endpointColumns = new Set(spec.endpoints.map((e) => e.column))
  const mapped = new Set(Object.keys(spec.propertyColumns ?? {}))
  const unmapped = columns.filter((c) => !endpointColumns.has(c) && !mapped.has(c))

  if (unmapped.length > 0) {
    throw new ImportValidationError(
      IMPORT_ERROR_CODES.LINK_COLUMNS_UNMAPPED,
      `columns not mapped to relationship properties: ${unmapped.join(', ')}`
    )
  }
}

export function suggestRelationshipType(spec: Pick<ImportLinkSpec, 'endpoints'>): string {
  return spec.endpoints
    .map((e) => e.label)
    .join('_')
    .toUpperCase()
}
