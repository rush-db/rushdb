export interface ImportUnitLocation {
  line?: number
  byteOffsetHint?: number
}

export interface ImportUnit {
  ordinal: number
  location: ImportUnitLocation
  value: Record<string, unknown>
}

export interface ParseContext {
  /** Stop after this many units; the caller flushes batches between pulls. */
  maxUnits?: number
  maxUnitBytes?: number
  /** Total source byte budget for formats that must materialize logical values. */
  maxSourceBytes?: number
  signal?: AbortSignal
}

import type { ImportFileFormat } from '../domain/import-run.types'

export interface ImportParserInspection {
  columns?: string[]
  jsonShape?: 'array' | 'single_object'
  format: ImportFileFormat
}

export interface ImportParser {
  inspect(input: Buffer): Promise<ImportParserInspection>
  /**
   * Streams logical root units from source bytes. Implementations must honor
   * backpressure and AbortSignal and never buffer the full source.
   */
  parse(input: NodeJS.ReadableStream, context: ParseContext): Promise<AsyncIterable<ImportUnit>>
}

export const IMPORT_PARSER_PORT = Symbol('IMPORT_PARSER_PORT')
