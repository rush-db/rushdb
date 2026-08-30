import { createInterface } from 'node:readline'

import type { ImportParser, ImportParserInspection, ImportUnit, ParseContext } from './import-parser.port'

/**
 * Streaming JSONL/NDJSON parser: one object per non-empty line, BOM accepted,
 * bounded line size, stable source lines. NDJSON is an alias of JSONL.
 */
export class JsonLinesImportParser implements ImportParser {
  public readonly format = 'jsonl' as const

  async inspect(input: Buffer): Promise<ImportParserInspection> {
    return { format: 'jsonl' }
  }

  parse(input: NodeJS.ReadableStream, context: ParseContext): Promise<AsyncIterable<ImportUnit>> {
    return Promise.resolve(this.iterate(input, context))
  }

  private async *iterate(input: NodeJS.ReadableStream, context: ParseContext): AsyncIterable<ImportUnit> {
    const maxLineBytes = context.maxUnitBytes ?? 8 * 1024 * 1024
    const rl = createInterface({ input: input as NodeJS.ReadableStream, crlfDelay: Infinity })

    let ordinal = 0
    let lineNumber = 0
    let sawFirstLine = false

    try {
      for await (const rawLine of rl) {
        if (context.signal?.aborted) {
          return
        }

        let line = rawLine
        if (!sawFirstLine && line.charCodeAt(0) === 0xfeff) {
          line = line.slice(1)
        }
        lineNumber += 1

        if (line.trim().length === 0) {
          continue
        }
        sawFirstLine = true

        if (Buffer.byteLength(line, 'utf8') > maxLineBytes) {
          throw new Error(`JSONL line ${lineNumber} exceeds the configured maximum logical unit size`)
        }

        let parsed: unknown
        try {
          parsed = JSON.parse(line)
        } catch (error) {
          throw new Error(`invalid JSON on line ${lineNumber}: ${(error as Error).message}`)
        }

        if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
          throw new Error(`line ${lineNumber} must be a JSON object`)
        }

        yield { ordinal: ordinal++, location: { line: lineNumber }, value: parsed as Record<string, unknown> }

        if (context.maxUnits && ordinal >= context.maxUnits) {
          return
        }
      }
    } finally {
      rl.close()
      cleanupStream(input)
    }
  }
}

function cleanupStream(input: NodeJS.ReadableStream): void {
  const destroyable = input as unknown as { destroy?: () => void }
  destroyable.destroy?.()
}
