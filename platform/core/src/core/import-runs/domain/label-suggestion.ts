import type { ImportFileFormat } from './import-run.types'

const LABEL_EXCEPTIONS: Record<string, string> = {
  people: 'PEOPLE',
  data: 'DATA',
  children: 'CHILDREN'
}

function singularize(word: string): string {
  if (LABEL_EXCEPTIONS[word]) {
    return LABEL_EXCEPTIONS[word]
  }
  if (word.length > 3 && word.endsWith('ies')) {
    return `${word.slice(0, -3)}y`
  }
  if (word.length > 3 && word.endsWith('ses')) {
    return word.slice(0, -2)
  }
  if (word.length > 2 && word.endsWith('s') && !word.endsWith('ss') && !word.endsWith('us')) {
    return word.slice(0, -1)
  }
  return word
}

/**
 * Deterministic filename -> label suggestion. Never authoritative.
 * "users.csv" -> USER; "order-items.jsonl" -> ORDER_ITEM; "char_ep.csv" -> CHAR_EP.
 */
export function suggestLabelFromFileName(fileName: string): string {
  const base = fileName.replace(/\.[^.]+$/, '')
  const words = base
    .replace(/([a-z])([A-Z])/g, '$1 $2')
    .split(/[\s._\-]+/)
    .filter(Boolean)
  const normalized = words.map((w) => w.toLowerCase())
  const last = normalized.length - 1
  const processed = normalized.map((word, i) => (i === last ? singularize(word) : word))
  return processed.join('_').toUpperCase()
}

export function detectFormatFromFileName(fileName: string): ImportFileFormat | null {
  const lower = fileName.toLowerCase()
  if (lower.endsWith('.csv')) {
    return 'csv'
  }
  if (lower.endsWith('.jsonl')) {
    return 'jsonl'
  }
  if (lower.endsWith('.ndjson')) {
    return 'ndjson'
  }
  if (lower.endsWith('.json')) {
    return 'json'
  }
  if (lower.endsWith('.parquet')) {
    return 'parquet'
  }
  return null
}

const REFERENCE_COLUMN_PATTERN = /(^|_)(id|ids|key|keys|code|guid|uuid|ref|fk)$/i

export function looksLikeReferenceColumn(columnName: string): boolean {
  return REFERENCE_COLUMN_PATTERN.test(columnName.trim())
}

export interface RoleSuggestionInput {
  columns: string[]
  fileName?: string
  siblingIdColumns?: string[]
}

export interface RoleSuggestion {
  role: 'records' | 'links'
  reason: string
}

/**
 * Advisory-only role suggestion. Two reference-like columns with nothing else,
 * optionally matching sibling id-like columns, suggests a pure join table.
 * Value-rich files are never suggested as links. Filename hints never decide alone.
 */
export function suggestRole({ columns, siblingIdColumns = [] }: RoleSuggestionInput): RoleSuggestion {
  if (columns.length !== 2) {
    return { role: 'records', reason: `expected exactly two reference-like columns, found ${columns.length}` }
  }

  const referenceLike = columns.filter((c) => looksLikeReferenceColumn(c))
  const matchesSibling =
    siblingIdColumns.length > 0 && columns.some((c) => siblingIdColumns.includes(c.toLowerCase()))

  if (
    referenceLike.length === columns.length &&
    (referenceLike.length === columns.length || matchesSibling)
  ) {
    return { role: 'links', reason: 'all columns look like foreign-key references' }
  }

  return { role: 'records', reason: 'file contains value-bearing columns' }
}
