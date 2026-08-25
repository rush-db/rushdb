import { chain } from 'stream-chain'
// stream-json ships CJS modules without esModuleInterop support in this package
// eslint-disable-next-line @typescript-eslint/no-require-imports
import Parser = require('stream-json')
// eslint-disable-next-line @typescript-eslint/no-require-imports
import StreamArray = require('stream-json/streamers/StreamArray')
// eslint-disable-next-line @typescript-eslint/no-require-imports
import StreamValues = require('stream-json/streamers/StreamValues')

import { PassThrough } from 'node:stream'

import type { ImportParser, ImportParserInspection, ImportUnit, ParseContext } from './import-parser.port'
import type { ImportFileFormat } from '../domain/import-run.types'

/**
 * Top-level JSON parser built on stream-json.
 *
 * - A top-level array is streamed element by element (bounded memory regardless
 *   of file size).
 * - A single top-level object (or any other single root value) is emitted as one
 *   logical unit; the whole value must fit the configured byte budget, otherwise
 *   parsing fails with an honest shape error instead of exhausting memory.
 *
 * The shape is probed from the first non-whitespace byte; the remainder of the
 * source streams through untouched.
 */
export class JsonObjectImportParser implements ImportParser {
  public readonly format: ImportFileFormat = 'json'

  async inspect(input: Buffer): Promise<ImportParserInspection> {
    const text = stripBom(input.subarray(0, Math.min(input.length, 64 * 1024)).toString('utf8')).trimStart()
    return { format: 'json', jsonShape: text.startsWith('[') ? 'array' : 'single_object' }
  }

  parse(input: NodeJS.ReadableStream, context: ParseContext): Promise<AsyncIterable<ImportUnit>> {
    return Promise.resolve(this.iterate(input, context))
  }

  private async *iterate(input: NodeJS.ReadableStream, context: ParseContext): AsyncIterable<ImportUnit> {
    const maxTotalBytes = context.maxSourceBytes ?? Number.MAX_SAFE_INTEGER

    const gate = new PassThrough()
    let firstChunk = await readFirstChunk(input)
    if (context.signal?.aborted) {
      cleanupStream(input)
      return
    }
    if (firstChunk === null) {
      cleanupStream(input)
      throw new Error('IMPORT_PARSE_ERROR: empty JSON source')
    }

    const shapeChar = firstNonWhitespaceChar(firstChunk)
    if (!shapeChar) {
      // whitespace-only leading chunk; keep scanning without wiring yet
      firstChunk = null
    }

    const isArray = shapeChar === '['
    const disassembler = isArray ? StreamArray.streamArray() : StreamValues.streamValues()
    const pipeline = chain([gate, Parser(), disassembler])

    const pumpFailure = new Promise<never>((_resolve, reject) => {
      pipeline.once('error', (error: Error) => reject(error))
      input.once('error', (error: Error) => reject(error))
    })

    const pump = (async () => {
      try {
        if (firstChunk !== null) {
          if (!gate.write(firstChunk)) {
            await onceDrain(gate)
          }
        }
        for await (const chunk of input as AsyncIterable<Buffer>) {
          if (context.signal?.aborted) {
            break
          }
          if (!gate.write(Buffer.from(chunk))) {
            await onceDrain(gate)
          }
        }
        gate.end()
      } catch {
        gate.destroy()
      }
    })()

    try {
      let ordinal = 0
      let totalBytes = 0

      const downstream = (async function* () {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        for await (const entry of pipeline as any) {
          const value = entry?.value
          if (!value || typeof value !== 'object' || Array.isArray(value)) {
            throw new Error(`IMPORT_PARSE_ERROR: JSON element ${ordinal} must be an object`)
          }

          totalBytes += JSON.stringify(value)?.length ?? 0
          if (totalBytes > maxTotalBytes) {
            throw new Error(
              `IMPORT_JSON_SHAPE_NOT_STREAMABLE: JSON logical value exceeds the configured ${maxTotalBytes} byte budget`
            )
          }

          yield { ordinal, location: {}, value: value as Record<string, unknown> }
          ordinal += 1

          if (context.maxUnits && ordinal >= context.maxUnits) {
            return
          }
        }
      })()

      yield* raceIterator(downstream, pumpFailure)
      await pump
    } finally {
      pipeline.destroy()
      gate.destroy()
      cleanupStream(input)
    }
  }
}

function readFirstChunk(input: NodeJS.ReadableStream): Promise<Buffer | null> {
  return new Promise((resolve) => {
    const onData = (chunk: Buffer | string) => {
      cleanup()
      resolve(typeof chunk === 'string' ? Buffer.from(chunk) : Buffer.from(chunk))
    }
    const onEnd = () => {
      cleanup()
      resolve(null)
    }
    const onError = () => {
      cleanup()
      resolve(null)
    }
    const cleanup = () => {
      input.off('data', onData)
      input.off('end', onEnd)
      input.off('error', onError)
    }
    input.once('data', onData)
    input.once('end', onEnd)
    input.once('error', onError)
  })
}

function firstNonWhitespaceChar(buffer: Buffer): string | null {
  const text = stripBom(buffer.toString('utf8'))
  for (const char of text) {
    if (!/\s/.test(char)) {
      return char
    }
  }
  return null
}

function onceDrain(stream: PassThrough): Promise<void> {
  return new Promise((resolve) => stream.once('drain', resolve))
}

const FAILURE_SENTINEL = Symbol('json-parse-failure')

async function* raceIterator<T>(
  iterator: AsyncIterable<T>,
  failure: Promise<never>
): AsyncGenerator<T, void, undefined> {
  const failureTagged = failure.then(() => FAILURE_SENTINEL)

  const iteratorHandle = iterator[Symbol.asyncIterator]()
  while (true) {
    const result = (await Promise.race([iteratorHandle.next(), failureTagged])) as
      | IteratorResult<T>
      | typeof FAILURE_SENTINEL
    if (result === FAILURE_SENTINEL) {
      return
    }
    if (result.done) {
      return
    }
    yield result.value
  }
}

function cleanupStream(input: NodeJS.ReadableStream): void {
  const destroyable = input as unknown as { destroy?: () => void }
  destroyable.destroy?.()
}

function stripBom(text: string): string {
  return text.charCodeAt(0) === 0xfeff ? text.slice(1) : text
}
