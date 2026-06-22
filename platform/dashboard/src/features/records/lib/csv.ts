type Row = Record<string, unknown>

/**
 * Result rows can arrive either as plain shaped objects or wrapped as `{ data: {...} }`
 * (mirrors the unwrap logic used by RecordsTable). Normalize to the inner object.
 */
export function unwrapRow(record: unknown): Row {
  if (
    record &&
    typeof record === 'object' &&
    'data' in record &&
    (record as { data?: unknown }).data &&
    typeof (record as { data?: unknown }).data === 'object'
  ) {
    return (record as { data: Row }).data
  }

  return (record ?? {}) as Row
}

// Columns are inferred from the union of keys across all rows, preserving first-seen
// order so the CSV header matches the order values appear in the result.
function inferColumns(rows: Row[]): string[] {
  const columns: string[] = []

  for (const row of rows) {
    for (const key of Object.keys(row)) {
      if (key === '__proptypes') continue
      if (!columns.includes(key)) {
        columns.push(key)
      }
    }
  }

  return columns
}

function formatCell(value: unknown): string {
  if (value === null || typeof value === 'undefined') return ''
  if (typeof value === 'object') return JSON.stringify(value)
  return String(value)
}

// RFC 4180 escaping: wrap in quotes and double up inner quotes when the value
// contains a quote, comma, or newline.
function escapeCell(value: string): string {
  if (/[",\r\n]/.test(value)) {
    return `"${value.replace(/"/g, '""')}"`
  }
  return value
}

/**
 * Build a CSV string from result rows (client-side, used for aggregated/shaped results).
 * Returns an empty string when there is nothing to export.
 */
export function rowsToCsv(records: unknown[]): string {
  const rows = records.map(unwrapRow)
  if (!rows.length) return ''

  const columns = inferColumns(rows)
  if (!columns.length) return ''

  const header = columns.map(escapeCell).join(',')
  const lines = rows.map((row) => columns.map((column) => escapeCell(formatCell(row[column]))).join(','))

  return [header, ...lines].join('\r\n')
}

/**
 * Trigger a browser download for the given CSV content.
 */
export function downloadCsv(filename: string, content: string): void {
  // Prepend a BOM so Excel detects UTF-8.
  const blob = new Blob(['﻿', content], { type: 'text/csv;charset=utf-8;' })
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.setAttribute('href', url)
  anchor.setAttribute('download', filename)
  anchor.click()
  anchor.remove()
  URL.revokeObjectURL(url)
}
