import type { ImportParser, ImportParserInspection, ImportUnit, ParseContext } from './import-parser.port'

const PARQUET_MAGIC = 'PAR1'

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type HyparquetModule = any

/**
 * hyparquet is ESM-only while this package compiles to CommonJS. A native
 * dynamic import survives both ts-jest and the nest build.
 */
let hyparquetModule: HyparquetModule | null = null

async function loadHyparquet(): Promise<HyparquetModule> {
  if (!hyparquetModule) {
    const nativeImport = new Function('specifier', 'return import(specifier)') as (
      specifier: string
    ) => Promise<HyparquetModule>
    hyparquetModule = await nativeImport('hyparquet')
  }
  return hyparquetModule
}

/**
 * Parquet reader backed by hyparquet (pure JS; snappy built in).
 *
 * Columnar formats cannot be streamed row-by-row without full page decode, so
 * the source is materialized up to a hard byte cap (RUSHDB_IMPORT_MAX_PARQUET_BYTES,
 * default 256 MiB) and rejected beyond it. Row order is stable, which keeps
 * replay ordinals — and therefore deterministic record IDs — stable.
 */
export class ParquetImportParser implements ImportParser {
  public readonly format = 'parquet' as const

  async inspect(input: Buffer): Promise<ImportParserInspection> {
    return { format: 'parquet' }
  }

  async parse(input: NodeJS.ReadableStream, context: ParseContext): Promise<AsyncIterable<ImportUnit>> {
    const bytes = await this.readAll(input, context)
    assertParquetMagic(bytes)

    const hyparquet = await loadHyparquet()
    const asyncBuffer = {
      byteLength: bytes.byteLength,
      // hyparquet wraps slices in DataView, which requires standalone ArrayBuffers
      slice: async (start: number, end?: number) => {
        const sub = bytes.subarray(start, end ?? bytes.byteLength)
        return sub.buffer.slice(sub.byteOffset, sub.byteOffset + sub.byteLength)
      }
    }
    const rows = (await hyparquet.parquetReadObjects({ file: asyncBuffer })) as Array<Record<string, unknown>>

    async function* iterate(): AsyncIterable<ImportUnit> {
      for (let ordinal = 0; ordinal < rows.length; ordinal++) {
        if (context.signal?.aborted) {
          return
        }
        const rawValue = rows[ordinal]
        if (!rawValue || typeof rawValue !== 'object') {
          throw new Error(`IMPORT_PARSE_ERROR: parquet row ${ordinal} is not an object`)
        }
        // Normalize parquet-native types to JSON-friendly values.
        const value = normalizeParquetValue(rawValue) as Record<string, unknown>
        yield { ordinal, location: {}, value }
        if (context.maxUnits && ordinal + 1 >= context.maxUnits) {
          return
        }
      }
    }

    return iterate()
  }

  private readAll(input: NodeJS.ReadableStream, context: ParseContext): Promise<Buffer> {
    const maxBytes = context.maxSourceBytes ?? 256 * 1024 * 1024

    return new Promise((resolve, reject) => {
      const chunks: Buffer[] = []
      let total = 0
      let settled = false

      const settle = (fn: () => void) => {
        if (!settled) {
          settled = true
          fn()
        }
      }

      input.on('data', (chunk: Buffer) => {
        total += chunk.byteLength
        if (total > maxBytes) {
          cleanup()
          reject(new Error(`IMPORT_LOGICAL_UNIT_TOO_LARGE: parquet source exceeds ${maxBytes} bytes`))
          return
        }
        chunks.push(Buffer.from(chunk))
      })

      const onEnd = () => {
        cleanup()
        settle(() => resolve(Buffer.concat(chunks)))
      }

      const onError = (error: Error) => {
        cleanup()
        settle(() => reject(error))
      }

      const cleanup = () => {
        input.removeAllListeners('data')
        input.off('end', onEnd)
        input.off('error', onError)
      }

      input.on('end', onEnd)
      input.on('error', onError)
    })
  }
}

function normalizeParquetValue(value: unknown): unknown {
  if (typeof value === 'bigint') {
    return value <= BigInt(Number.MAX_SAFE_INTEGER) ? Number(value) : value.toString()
  }
  if (value instanceof Date) {
    return value.toISOString()
  }
  if (value instanceof Uint8Array) {
    return Buffer.from(value).toString('base64')
  }
  if (Array.isArray(value)) {
    return value.map(normalizeParquetValue)
  }
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, normalizeParquetValue(v)]))
  }
  return value
}

function assertParquetMagic(bytes: Buffer): void {
  if (
    bytes.length < 8 ||
    bytes.subarray(0, 4).toString('ascii') !== PARQUET_MAGIC ||
    bytes.subarray(bytes.length - 4).toString('ascii') !== PARQUET_MAGIC
  ) {
    throw new Error('IMPORT_FORMAT_UNSUPPORTED: not a valid parquet file (missing PAR1 magic)')
  }
}
