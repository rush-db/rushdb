import { createReadStream, existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { PassThrough } from 'node:stream'

import { JsonLinesImportParser } from './parser/json-lines.parser'
import { JsonObjectImportParser } from './parser/json-object.parser'
import { ParquetImportParser } from './parser/parquet.parser'
import { LocalImportStorageAdapter } from './storage/local-import-storage.adapter'

async function collectUnits(iterable: AsyncIterable<{ ordinal: number; value: Record<string, unknown> }>) {
  const units = []
  for await (const unit of iterable) {
    units.push(unit)
  }
  return units
}

function streamFrom(text: string): NodeJS.ReadableStream {
  const stream = new PassThrough()
  stream.end(Buffer.from(text, 'utf8'))
  return stream
}

describe('JsonObjectImportParser', () => {
  const parser = new JsonObjectImportParser()

  it('streams top-level array elements as logical units', async () => {
    const json = '[{"id":1},{"id":2},{"id":3}]'
    const units = await collectUnits(await parser.parse(streamFrom(json), {}))

    expect(units.map((u) => u.value.id)).toEqual([1, 2, 3])
    expect(units.map((u) => u.ordinal)).toEqual([0, 1, 2])
  })

  it('emits a single top-level object as one unit', async () => {
    const json = '{"id":1,"nested":{"label":"child"}}'
    const units = await collectUnits(await parser.parse(streamFrom(json), {}))

    expect(units).toHaveLength(1)
    expect(units[0].value.nested).toEqual({ label: 'child' })
  })

  it('reports the detected shape on inspect', async () => {
    await expect(parser.inspect(Buffer.from('[1]', 'utf8'))).resolves.toMatchObject({ jsonShape: 'array' })
    await expect(parser.inspect(Buffer.from('{"a":1}', 'utf8'))).resolves.toMatchObject({
      jsonShape: 'single_object'
    })
  })

  it('rejects scalar roots', async () => {
    await expect(collectUnits(await parser.parse(streamFrom('"text"'), {}))).rejects.toThrow(
      /must be an object/
    )
  })

  it('enforces the source byte budget for single objects', async () => {
    const json = '{"data":"' + 'x'.repeat(1000) + '"}'
    await expect(collectUnits(await parser.parse(streamFrom(json), { maxSourceBytes: 10 }))).rejects.toThrow(
      /IMPORT_JSON_SHAPE_NOT_STREAMABLE/
    )
  })
})

describe('JsonLinesImportParser (ndjson alias)', () => {
  const parser = new JsonLinesImportParser()

  it('parses ndjson content identically to jsonl', async () => {
    const units = await collectUnits(await parser.parse(streamFrom('{"id":1}\n{"id":2}\n'), {}))
    expect(units.map((u) => u.value.id)).toEqual([1, 2])
  })
})

describe('ParquetImportParser', () => {
  const parser = new ParquetImportParser()
  const fixturePath = join(__dirname, 'parser', '__fixtures__', 'characters.snappy.parquet')

  it('yields stable-ordinal row units from a snappy-compressed file', async () => {
    const bytes = readFileSync(fixturePath)
    const units = await collectUnits(await parser.parse(streamFromBytes(bytes), {}))

    expect(units).toHaveLength(3)
    expect(units[0].value).toMatchObject({ id: 1, name: 'Rick Sanchez' })
    expect(units[2].value).toMatchObject({ id: 3, name: 'Summer Smith' })
    expect(units.map((u) => u.ordinal)).toEqual([0, 1, 2])
  })

  it('reads uncompressed files', async () => {
    const bytes = readFileSync(join(__dirname, 'parser', '__fixtures__', 'characters.plain.parquet'))
    const units = await collectUnits(await parser.parse(streamFromBytes(bytes), {}))
    expect(units).toHaveLength(3)
  })

  it('reads from a local storage stream end-to-end', async () => {
    const adapter = new LocalImportStorageAdapter(mkdtempSync(join(tmpdir(), 'parquet-test-')))
    try {
      const initiated = await adapter.initiate('p1', 'f1', 1)
      await adapter.writeChunk(initiated.storageKey, readFileSync(fixturePath))
      await adapter.complete(initiated.storageKey)

      const stream = await adapter.readStream(initiated.storageKey)
      const units = await collectUnits(await parser.parse(stream, {}))
      expect(units).toHaveLength(3)
    } finally {
      rmSync(adapter['root'], { recursive: true, force: true })
    }
  })

  it('rejects non-parquet payloads via magic bytes', async () => {
    const run = async () =>
      collectUnits(await parser.parse(streamFromBytes(Buffer.from('definitely not parquet')), {}))
    await expect(run()).rejects.toThrow(/PAR1/)
  })

  function streamFromBytes(bytes: Buffer): NodeJS.ReadableStream {
    const stream = new PassThrough()
    stream.end(bytes)
    return stream
  }
})

describe('LocalImportStorageAdapter retention contract', () => {
  let root: string

  beforeEach(() => {
    root = mkdtempSync(join(tmpdir(), 'local-storage-test-'))
  })

  afterEach(() => {
    rmSync(root, { recursive: true, force: true })
  })

  it('writes, completes, streams, and deletes like the s3 backend', async () => {
    const adapter = new LocalImportStorageAdapter(root)

    const initiated = await adapter.initiate('p1', 'file-1', 1)
    expect(initiated.storageKey.startsWith('imports/p1/file-1/g1/')).toBe(true)

    await adapter.writeChunk(initiated.storageKey, Buffer.from('chunk-one,'))
    await adapter.writeChunk(initiated.storageKey, Buffer.from('chunk-two'))
    const meta = await adapter.complete(initiated.storageKey, 19)
    expect(meta.sizeBytes).toBe(19)

    const head = await adapter.head(initiated.storageKey)
    expect(head?.sizeBytes).toBe(19)
    expect(existsSync(join(root, initiated.storageKey))).toBe(true)

    await adapter.delete(initiated.storageKey)
    await expect(adapter.head(initiated.storageKey)).resolves.toBeNull()
  })

  it('rejects path traversal in keys', async () => {
    const adapter = new LocalImportStorageAdapter(root)
    await expect(adapter.writeChunk('../../etc/passwd', Buffer.from('x'))).rejects.toThrow(
      /invalid storage key/
    )
  })

  it('supports streamed reads compatible with parsers', async () => {
    const adapter = new LocalImportStorageAdapter(root)
    const initiated = await adapter.initiate('p1', 'file-2', 1)
    await adapter.writeChunk(initiated.storageKey, Buffer.from('{"id":1}\n{"id":2}\n'))
    await adapter.complete(initiated.storageKey)

    const parser = new JsonLinesImportParser()
    const units = await collectUnits(
      await parser.parse(createReadStream(join(root, initiated.storageKey)), {})
    )
    expect(units).toHaveLength(2)
  })
})
