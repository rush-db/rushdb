/**
 * End-to-end coverage for the AI schema endpoints (`POST /ai/schema`, `POST /ai/schema/md`)
 * after the labels-first recalculation restructure: per-label edge-count property
 * inventory, exact number/datetime min/max, bounded string/boolean sampling (values +
 * isArray), split relationship topology/property queries, cache serving and the
 * post-write side-effect refresh — asserted against an exact expected schema for a
 * crafted dataset in an isolated project.
 */
import RushDB from '../packages/javascript-sdk/src/index.node'

import { provisionApiKey } from './setup/bootstrap'
import { ADMIN_LOGIN, ADMIN_PASSWORD, BASE_URL } from './setup/env'

const API = `${BASE_URL}/api/v1`
const RUN = Date.now().toString(36)

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms))

type SchemaProperty = {
  id: string
  name: string
  type: string
  recordsCount?: number
  isArray?: boolean
  values?: Array<string | number | boolean>
  min?: number | string
  max?: number | string
}

type SchemaRelationship = {
  label: string
  type: string
  direction: 'in' | 'out'
  count: number
  properties?: Array<{
    name: string
    type: string
    relationshipsCount: number
    min?: number | string
    max?: number | string
    values?: unknown[]
  }>
}

type SchemaItem = {
  label: string
  count: number
  properties: SchemaProperty[]
  relationships: SchemaRelationship[]
}

describe('AI schema recalculation (e2e)', () => {
  let db: InstanceType<typeof RushDB>
  let apiKey: string

  const api = async (path: string, body?: unknown): Promise<any> => {
    const response = await fetch(`${API}${path}`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${apiKey}`
      },
      ...(body !== undefined ? { body: JSON.stringify(body) } : { body: '{}' })
    })
    const json: any = await response.json().catch(() => ({}))
    if (!response.ok) {
      throw new Error(`POST ${path} -> ${response.status}: ${JSON.stringify(json)}`)
    }
    return json.data ?? json
  }

  const BOOKS = [
    {
      title: 'Dune',
      authorName: 'Frank Herbert',
      pages: 412,
      published: '1965-08-01T00:00:00.000Z',
      inPrint: true,
      tags: ['sci-fi', 'classic'],
      ratings: [4.5, 4.8]
    },
    {
      title: 'Hyperion',
      authorName: 'Dan Simmons',
      pages: 482,
      published: '1989-05-26T00:00:00.000Z',
      inPrint: true,
      tags: ['sci-fi', 'space-opera'],
      ratings: [4.2, 4.6]
    },
    {
      title: 'The Santaroga Barrier',
      authorName: 'Frank Herbert',
      pages: 255,
      published: '1968-10-01T00:00:00.000Z',
      inPrint: false,
      tags: ['sci-fi'],
      ratings: [4.0]
    },
    {
      title: 'Endymion',
      authorName: 'Dan Simmons',
      pages: 204,
      published: '1996-01-15T00:00:00.000Z',
      inPrint: true,
      tags: ['space-opera'],
      ratings: [3.9, 4.1]
    }
  ]

  const AUTHORS = [
    { name: 'Frank Herbert', born: 1920 },
    { name: 'Dan Simmons', born: 1948 }
  ]

  jest.setTimeout(170_000)

  beforeAll(async () => {
    const provisioned = await provisionApiKey({
      baseUrl: BASE_URL,
      login: ADMIN_LOGIN,
      password: ADMIN_PASSWORD,
      projectName: `e2e-ai-schema-${RUN}`
    })
    apiKey = provisioned.apiKey
    db = new RushDB(apiKey, { url: BASE_URL })

    await db.records.createMany({
      label: 'BOOK',
      data: BOOKS,
      options: { suggestTypes: true, returnResult: false }
    })
    await db.records.createMany({
      label: 'AUTHOR',
      data: AUTHORS,
      options: { suggestTypes: true, returnResult: false }
    })

    // AUTHOR -[WROTE]-> BOOK with per-author relationship properties so the schema's
    // relationship-property summaries have a real min/max spread.
    await db.relationships.createMany({
      source: { label: 'AUTHOR', key: 'name', where: { name: 'Frank Herbert' } },
      target: { label: 'BOOK', key: 'authorName', where: { authorName: 'Frank Herbert' } },
      type: 'WROTE',
      direction: 'out',
      properties: { royalty: 0.25, since: '2021-06-01T00:00:00.000Z' }
    })
    await db.relationships.createMany({
      source: { label: 'AUTHOR', key: 'name', where: { name: 'Dan Simmons' } },
      target: { label: 'BOOK', key: 'authorName', where: { authorName: 'Dan Simmons' } },
      type: 'WROTE',
      direction: 'out',
      properties: { royalty: 0.1, since: '2019-03-15T00:00:00.000Z' }
    })

    // relationships.createMany materializes via apoc.periodic.iterate, which can land
    // just after the response — wait until all 4 WROTE relationships are visible.
    for (let attempt = 0; attempt < 20; attempt++) {
      const found = await db.relationships.find({
        source: { labels: ['AUTHOR'] },
        target: { labels: ['BOOK'] },
        where: { type: 'WROTE' },
        limit: 100
      })
      if ((found.data ?? []).length >= 4) {
        return
      }
      await sleep(1000)
    }
    throw new Error('WROTE relationships did not materialize in time')
  })

  const sortedByName = <T extends { name: string }>(items: T[] = []): T[] =>
    [...items].sort((a, b) => a.name.localeCompare(b.name))

  const sortedValues = (values: unknown[] = []) => [...values].map(String).sort()

  let schema: SchemaItem[]

  it('recalculates the full schema on force', async () => {
    schema = await api('/ai/schema', { force: true })

    expect(Array.isArray(schema)).toBe(true)
    // Sorted by record count descending
    expect(schema.map((item) => item.label)).toEqual(['BOOK', 'AUTHOR'])
    expect(schema[0].count).toBe(4)
    expect(schema[1].count).toBe(2)
  })

  it('BOOK properties: exact recordsCount, min/max, isArray, samples', () => {
    const book = schema.find((item) => item.label === 'BOOK')!
    const props = sortedByName(book.properties)

    expect(props.map((p) => p.name)).toEqual([
      'authorName',
      'inPrint',
      'pages',
      'published',
      'ratings',
      'tags',
      'title'
    ])

    const byName = new Map(props.map((p) => [p.name, p]))

    const authorName = byName.get('authorName')!
    expect(authorName.type).toBe('string')
    expect(authorName.recordsCount).toBe(4)
    expect(authorName.isArray).toBeUndefined()
    expect(sortedValues(authorName.values)).toEqual(['Dan Simmons', 'Frank Herbert'])

    const inPrint = byName.get('inPrint')!
    expect(inPrint.type).toBe('boolean')
    expect(inPrint.recordsCount).toBe(4)
    expect(inPrint.values).toEqual(['true', 'false'])
    expect(inPrint.isArray).toBeUndefined()

    const pages = byName.get('pages')!
    expect(pages.type).toBe('number')
    expect(pages.recordsCount).toBe(4)
    expect(pages.min).toBe(204)
    expect(pages.max).toBe(482)
    expect(pages.isArray).toBeUndefined()

    const published = byName.get('published')!
    expect(published.type).toBe('datetime')
    expect(published.recordsCount).toBe(4)
    // datetime min/max are normalized through Cypher datetime()/toString()
    expect(published.min).toBe('1965-08-01T00:00:00Z')
    expect(published.max).toBe('1996-01-15T00:00:00Z')

    const ratings = byName.get('ratings')!
    expect(ratings.type).toBe('number')
    expect(ratings.recordsCount).toBe(4)
    expect(ratings.isArray).toBe(true)
    // min/max are exact across flattened array values
    expect(ratings.min).toBe(3.9)
    expect(ratings.max).toBe(4.8)

    const tags = byName.get('tags')!
    expect(tags.type).toBe('string')
    expect(tags.recordsCount).toBe(4)
    expect(tags.isArray).toBe(true)
    expect(sortedValues(tags.values)).toEqual(['classic', 'sci-fi', 'space-opera'])

    const title = byName.get('title')!
    expect(title.type).toBe('string')
    expect(title.recordsCount).toBe(4)
    expect(sortedValues(title.values)).toEqual(['Dune', 'Endymion', 'Hyperion', 'The Santaroga Barrier'])
  })

  it('AUTHOR properties: exact recordsCount and number range', () => {
    const author = schema.find((item) => item.label === 'AUTHOR')!
    const props = sortedByName(author.properties)

    expect(props.map((p) => p.name)).toEqual(['born', 'name'])

    const born = props[0]
    expect(born.type).toBe('number')
    expect(born.recordsCount).toBe(2)
    expect(born.min).toBe(1920)
    expect(born.max).toBe(1948)

    const name = props[1]
    expect(name.type).toBe('string')
    expect(name.recordsCount).toBe(2)
    expect(sortedValues(name.values)).toEqual(['Dan Simmons', 'Frank Herbert'])
  })

  it('relationship topology: exact counts in both directions with property summaries', () => {
    const author = schema.find((item) => item.label === 'AUTHOR')!
    const book = schema.find((item) => item.label === 'BOOK')!

    expect(author.relationships).toHaveLength(1)
    const outgoing = author.relationships[0]
    expect(outgoing.label).toBe('BOOK')
    expect(outgoing.type).toBe('WROTE')
    expect(outgoing.direction).toBe('out')
    expect(outgoing.count).toBe(4)

    expect(book.relationships).toHaveLength(1)
    const incoming = book.relationships[0]
    expect(incoming.label).toBe('AUTHOR')
    expect(incoming.type).toBe('WROTE')
    expect(incoming.direction).toBe('in')
    expect(incoming.count).toBe(4)

    for (const rel of [outgoing, incoming]) {
      const relProps = sortedByName(rel.properties ?? [])
      expect(relProps.map((p) => p.name)).toEqual(['royalty', 'since'])

      const royalty = relProps[0]
      expect(royalty.type).toBe('number')
      expect(royalty.relationshipsCount).toBe(4)
      expect(royalty.min).toBe(0.1)
      expect(royalty.max).toBe(0.25)

      const since = relProps[1]
      expect(since.type).toBe('datetime')
      expect(since.relationshipsCount).toBe(4)
      // Relationship datetime min/max are lexicographic over the stored ISO strings
      expect(since.min).toBe('2019-03-15T00:00:00.000Z')
      expect(since.max).toBe('2021-06-01T00:00:00.000Z')
    }
  })

  it('serves the cached schema identically without force', async () => {
    const cached: SchemaItem[] = await api('/ai/schema', {})

    const normalize = (items: SchemaItem[]) =>
      items.map((item) => ({
        ...item,
        properties: sortedByName(item.properties).map((p) => ({
          ...p,
          values: p.values ? sortedValues(p.values) : undefined
        })),
        relationships: item.relationships.map((rel) => ({
          ...rel,
          properties: sortedByName(rel.properties ?? [])
        }))
      }))

    expect(normalize(cached)).toEqual(normalize(schema))
  })

  it('applies the labels filter in-memory', async () => {
    const filtered: SchemaItem[] = await api('/ai/schema', { labels: ['AUTHOR'] })
    expect(filtered.map((item) => item.label)).toEqual(['AUTHOR'])
  })

  it('renders the markdown variant from the same schema', async () => {
    const md: string = await api('/ai/schema/md', {})

    expect(typeof md).toBe('string')
    expect(md).toContain('## `BOOK` (4 records)')
    expect(md).toContain('## `AUTHOR` (2 records)')
    expect(md).toContain('`204`..`482`')
    expect(md).toContain('`1965-08-01T00:00:00Z`..`1996-01-15T00:00:00Z`')
    // Array types are rendered with the [] suffix
    expect(md).toContain('| `tags` | string[] |')
    expect(md).toContain('| `ratings` | number[] |')
    // Relationship traversal table with edge properties
    expect(md).toContain('`royalty:number`')
    expect(md).toContain('`since:datetime`')
  })

  it('refreshes the cache via the post-write side effect (no force, no blocking)', async () => {
    await db.records.createMany({
      label: 'BOOK',
      data: [
        {
          title: 'Ilium',
          authorName: 'Dan Simmons',
          pages: 576,
          published: '2003-07-01T00:00:00.000Z',
          inPrint: true,
          tags: ['sci-fi'],
          ratings: [4.3]
        }
      ],
      options: { suggestTypes: true, returnResult: false }
    })

    // The RECALCULATE_SCHEMA_CACHE side effect runs post-commit in the background;
    // non-force reads serve the (possibly stale) cache immediately and must never block.
    let latest: SchemaItem[] = []
    for (let attempt = 0; attempt < 30; attempt++) {
      const started = Date.now()
      latest = await api('/ai/schema', {})
      // Serving must be cache-backed — a blocking full recompute here is a regression.
      expect(Date.now() - started).toBeLessThan(10_000)
      const book = latest.find((item) => item.label === 'BOOK')
      if (book?.count === 5) {
        break
      }
      await sleep(1000)
    }

    const book = latest.find((item) => item.label === 'BOOK')!
    expect(book.count).toBe(5)
    const pages = book.properties.find((p) => p.name === 'pages')!
    expect(pages.recordsCount).toBe(5)
    expect(pages.max).toBe(576)
  })
})
