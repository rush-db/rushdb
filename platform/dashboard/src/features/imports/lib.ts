import type { ImportFileFormat } from './types'

const SUPPORTED_EXTENSIONS = new Set(['csv', 'jsonl', 'ndjson', 'json', 'parquet'])

export const SUPPORTED_FORMATS: ImportFileFormat[] = ['csv', 'jsonl', 'ndjson', 'json', 'parquet']

export function isSupportedFileName(fileName: string): boolean {
  const ext = fileName.split('.').pop()?.toLowerCase()
  return Boolean(ext && SUPPORTED_EXTENSIONS.has(ext))
}

export function detectFormat(fileName: string): ImportFileFormat | null {
  const ext = fileName.split('.').pop()?.toLowerCase()
  if (!ext || !SUPPORTED_EXTENSIONS.has(ext)) return null
  return ext as ImportFileFormat
}

export function formatBytes(bytes: number): string {
  if (!bytes || bytes < 0) return '0 B'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`
}

let clientFileIdCounter = 0
export function generateClientFileId(): string {
  clientFileIdCounter += 1
  return `cf-${Date.now().toString(36)}-${clientFileIdCounter}-${Math.random().toString(36).slice(2, 8)}`
}

/** Uppercase base filename, non-alphanumerics collapsed to underscores. */
export function suggestLabel(fileName: string): string {
  const base = fileName.replace(/\.[^.]+$/, '')
  const cleaned = base
    .replace(/[^a-zA-Z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .toUpperCase()
  return cleaned || 'RECORD'
}

/**
 * Parses the first data row of a file into column names, when possible.
 * CSV/JSONL/NDJSON are supported; JSON and parquet return an empty list and
 * callers should fall back to free-text column entry.
 */
export async function readHeaderColumns(file: File): Promise<string[]> {
  const ext = file.name.split('.').pop()?.toLowerCase()

  if (ext === 'csv') {
    const text = await file.slice(0, 64 * 1024).text()
    const firstLine = text.replace(/^\uFEFF/, '').split(/\r?\n/, 1)[0] ?? ''
    return splitCsvLine(firstLine)
  }

  if (ext === 'jsonl' || ext === 'ndjson') {
    const text = await file.slice(0, 64 * 1024).text()
    const firstLine = text.split(/\r?\n/).find((l) => l.trim().length > 0)
    if (!firstLine) return []
    try {
      const parsed = JSON.parse(firstLine)
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
        return Object.keys(parsed)
      }
    } catch {
      return []
    }
  }

  return []
}

/** Minimal CSV line splitter that respects double-quoted fields. */
function splitCsvLine(line: string): string[] {
  const cells: string[] = []
  let current = ''
  let inQuotes = false

  for (let i = 0; i < line.length; i += 1) {
    const char = line[i]
    if (char === '"') {
      if (inQuotes && line[i + 1] === '"') {
        current += '"'
        i += 1
      } else {
        inQuotes = !inQuotes
      }
    } else if (char === ',' && !inQuotes) {
      cells.push(current.trim())
      current = ''
    } else {
      current += char
    }
  }
  cells.push(current.trim())
  return cells
}

const TERMINAL_STATUSES = new Set(['completed', 'completed_with_errors', 'failed', 'canceled'])

export function isTerminalStatus(status: string): boolean {
  return TERMINAL_STATUSES.has(status)
}

export function isRunActive(status: string): boolean {
  return !isTerminalStatus(status)
}
