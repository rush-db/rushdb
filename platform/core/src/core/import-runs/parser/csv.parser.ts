import { parse as papaParse } from 'papaparse'

import { StringDecoder } from 'node:string_decoder'

import type { ImportParser, ImportParserInspection, ImportUnit, ParseContext } from './import-parser.port'

/**
 * Streaming CSV parser with bounded memory: source bytes are consumed through a
 * quote-aware incremental record splitter, so quoted multiline fields survive
 * chunk boundaries while nothing accumulates beyond one logical record.
 *
 * Headers are sanitized deterministically (BOM stripped, null bytes removed,
 * empties -> column_N, duplicates suffixed). Values stay strings; dynamic typing
 * belongs to the normalization stage.
 */
export class CsvImportParser implements ImportParser {
  public readonly format = 'csv' as const

  async inspect(input: Buffer): Promise<ImportParserInspection> {
    const text = stripBom(input.subarray(0, Math.min(input.length, 64 * 1024)).toString('utf8'))
    const result = papaParse<string[]>(text, { header: false, skipEmptyLines: 'greedy' })
    const header = result.data[0]
    return {
      format: 'csv',
      columns: Array.isArray(header) ? header.map((c) => String(c ?? '').trim()) : undefined
    }
  }

  parse(input: NodeJS.ReadableStream, context: ParseContext): Promise<AsyncIterable<ImportUnit>> {
    return Promise.resolve(this.iterate(input, context))
  }

  private async *iterate(input: NodeJS.ReadableStream, context: ParseContext): AsyncIterable<ImportUnit> {
    const splitter = new CsvRecordSplitter()
    let ordinal = 0
    let lineNumber = 0
    let headerSanitized: string[] | null = null
    const decoder = new StringDecoder('utf8')
    const decodeChunk = (raw: unknown): string =>
      typeof raw === 'string' ? raw : decoder.write(Buffer.from(raw as Buffer))

    const flushDecoder = (): string => decoder.end()

    const processRecord = function* (rawRecord: string): Generator<ImportUnit> {
      if (rawRecord.trim().length === 0) {
        return
      }
      lineNumber += 1

      const parsed = papaParse<string[]>(rawRecord, { header: false })
      const fields = parsed.data[0]
      if (!Array.isArray(fields)) {
        return
      }

      if (!headerSanitized) {
        headerSanitized = sanitizeHeader(fields)
        return
      }

      const value: Record<string, unknown> = {}
      for (let i = 0; i < headerSanitized.length; i++) {
        value[headerSanitized[i]] = fields[i]
      }
      yield { ordinal, location: { line: lineNumber }, value }
    }

    try {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      for await (const rawChunk of input as AsyncIterable<any>) {
        if (context.signal?.aborted) {
          return
        }

        const chunk = decodeChunk(rawChunk)
        const records = splitter.push(chunk)

        for (const record of records) {
          for (const unit of processRecord(record)) {
            yield unit
            ordinal += 1
            if (context.maxUnits && ordinal >= context.maxUnits) {
              return
            }
          }
        }
      }

      const tail = splitter.finish() + flushDecoder()
      for (const unit of processRecord(tail)) {
        yield unit
        ordinal += 1
        if (context.maxUnits && ordinal >= context.maxUnits) {
          return
        }
      }
    } finally {
      cleanupStream(input)
    }
  }
}

function cleanupStream(input: NodeJS.ReadableStream): void {
  const destroyable = input as unknown as { destroy?: () => void }
  destroyable.destroy?.()
}

function stripBom(text: string): string {
  return text.charCodeAt(0) === 0xfeff ? text.slice(1) : text
}

function sanitizeHeader(raw: string[]): string[] {
  const seen = new Map<string, number>()
  return raw.map((name, index) => {
    let cleaned = String(name ?? '')
      .replace(/\0/g, '')
      .trim()
    if (cleaned.length === 0) {
      cleaned = `column_${index}`
    }
    const count = seen.get(cleaned) ?? 0
    seen.set(cleaned, count + 1)
    return count > 0 ? `${cleaned}_${count}` : cleaned
  })
}

/**
 * Incremental, quote-aware CSV record splitter. Feed it arbitrary text chunks;
 * it returns every complete logical record. Handles:
 * - CRLF/LF row separators;
 * - double-quoted fields containing delimiters, newlines and escaped quotes.
 */
export class CsvRecordSplitter {
  private buffer = ''
  private inQuotes = false

  push(chunk: string): string[] {
    this.buffer += chunk
    const records: string[] = []
    let recordStart = 0
    let index = 0

    while (index < this.buffer.length) {
      const char = this.buffer[index]

      if (this.inQuotes) {
        if (char === '"') {
          if (this.buffer[index + 1] === '"') {
            index += 2
            continue
          }
          this.inQuotes = false
        }
        index += 1
        continue
      }

      if (char === '"') {
        this.inQuotes = true
        index += 1
        continue
      }

      if (char === '\n') {
        records.push(this.buffer.slice(recordStart, index).replace(/\r$/, ''))
        index += 1
        recordStart = index
        continue
      }

      index += 1
    }

    this.buffer = this.buffer.slice(recordStart)
    return records
  }

  finish(): string {
    const rest = this.buffer
    this.buffer = ''
    this.inQuotes = false
    return rest
  }
}
