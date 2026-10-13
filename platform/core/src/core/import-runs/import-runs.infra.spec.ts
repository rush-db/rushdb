import { PassThrough } from 'node:stream'

import { CsvImportParser } from './parser/csv.parser'
import { JsonLinesImportParser } from './parser/json-lines.parser'
import { MemoryImportStorageAdapter } from './storage/memory-import-storage.adapter'

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

describe('CsvImportParser', () => {
  const parser = new CsvImportParser()

  it('emits sanitized headers and row objects', async () => {
    const csv = 'id,name,name\n1,Rick,Sanchez\n2,Morty,Smith\n'
    const units = await collectUnits(await parser.parse(streamFrom(csv), {}))

    expect(units).toHaveLength(2)
    expect(units[0].value).toEqual({ id: '1', name: 'Rick', name_1: 'Sanchez' })
    expect(units[1].value).toEqual({ id: '2', name: 'Morty', name_1: 'Smith' })
  })

  it('handles quoted multiline fields without splitting records', async () => {
    const csv = 'id,quote\n1,"line one\nline two"\n2,plain\n'
    const units = await collectUnits(await parser.parse(streamFrom(csv), {}))

    expect(units).toHaveLength(2)
    expect(units[0].value.quote).toBe('line one\nline two')
    expect(units[1].value.id).toBe('2')
  })

  it('reports columns on inspect without consuming the source', async () => {
    const inspection = await parser.inspect(Buffer.from('a,b\n1,2\n', 'utf8'))
    expect(inspection.format).toBe('csv')
    expect(inspection.columns).toEqual(['a', 'b'])
  })
})

describe('JsonLinesImportParser', () => {
  const parser = new JsonLinesImportParser()

  it('parses each non-empty line as an object with stable ordinals and lines', async () => {
    const jsonl = '{"id":1}\n\n{"id":2}\n{"id":3}'
    const units = await collectUnits(await parser.parse(streamFrom(jsonl), {}))

    expect(units.map((u) => u.value.id)).toEqual([1, 2, 3])
    expect(units.map((u) => u.ordinal)).toEqual([0, 1, 2])
    expect(units[2].location.line).toBe(4)
  })

  it('rejects non-object lines', async () => {
    await expect(collectUnits(await parser.parse(streamFrom('[1,2]\n'), {}))).rejects.toThrow(/JSON object/)
  })
})

describe('MemoryImportStorageAdapter', () => {
  const adapter = new MemoryImportStorageAdapter()

  it('stores, completes, streams and deletes objects', async () => {
    const initiated = await adapter.initiate('p1', 'f1', 1)
    await adapter.writeChunk(initiated.storageKey, Buffer.from('hello '))
    await adapter.writeChunk(initiated.storageKey, Buffer.from('world'))
    const meta = await adapter.complete(initiated.storageKey)

    expect(meta.sizeBytes).toBe(11)

    const chunks: Buffer[] = []
    for await (const chunk of (await adapter.readStream(initiated.storageKey)) as AsyncIterable<Buffer>) {
      chunks.push(chunk)
    }
    expect(Buffer.concat(chunks).toString()).toBe('hello world')

    await adapter.delete(initiated.storageKey)
    await expect(adapter.head(initiated.storageKey)).resolves.toBeNull()
  })
})
