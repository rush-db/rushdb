import { BadRequestException } from '@nestjs/common'

import { ValidationPipe } from '@/common/validation/validation.pipe'
import { createEmbeddingIndexSchema } from '@/core/ai/validation/schemas/embedding-index.schema'
import { createEntitySchema } from '@/core/entity/validation/schemas/create-entity.schema'
import { PropertyValuesPipe } from '@/core/property/validation/property-values.pipe'
import { propertySchema } from '@/core/property/validation/schemas/property.schema'
import { createRelationsByKeysSchema } from '@/core/relationships/validation/schemas/relations.schema'
import { searchSchema } from '@/core/search/validation/schemas/search.schema'

describe('zod migration smoke', () => {
  const pipeFor = (schema: any) => new (ValidationPipe(schema, 'body'))()
  const meta = { type: 'body' as const }

  it('createEntitySchema keeps unknown keys (vectors) through the pipe', () => {
    const body = {
      label: 'Car',
      properties: [{ type: 'number', name: 'year', value: 2024 }],
      vectors: [{ propertyName: 'embedding', vector: [0.1, 0.2] }]
    }
    const result = pipeFor(createEntitySchema).transform(body, meta)
    expect(result.vectors).toEqual(body.vectors)
    expect(result.properties).toEqual(body.properties)
  })

  it('createEntitySchema rejects mis-named payload key', () => {
    expect(() => pipeFor(createEntitySchema).transform({ label: 'Car', payload: {} }, meta)).toThrow(
      BadRequestException
    )
  })

  it('createEntitySchema accepts data-shaped payload with mixed values', () => {
    const body = { label: 'Car', data: { year: 2024, tags: ['a', 'b'], sold: null, ok: true } }
    expect(() => pipeFor(createEntitySchema).transform(body, meta)).not.toThrow()
  })

  it('propertySchema validates value against type conditionally', () => {
    expect(propertySchema.safeParse({ type: 'number', name: 'n', value: [1, 2] }).success).toBe(true)
    expect(propertySchema.safeParse({ type: 'number', name: 'n', value: 'oops' }).success).toBe(false)
    expect(propertySchema.safeParse({ type: 'boolean', name: 'b', value: true }).success).toBe(true)
    expect(propertySchema.safeParse({ type: 'datetime', name: 'd', value: 'not-a-date' }).success).toBe(false)
    expect(
      propertySchema.safeParse({ type: 'datetime', name: 'd', value: '2024-01-01T00:00:00Z' }).success
    ).toBe(true)
    // with a separator the raw (pre-split) value must be a plain string
    expect(
      propertySchema.safeParse({ type: 'number', name: 'n', value: '1;2;3', valueSeparator: ';' }).success
    ).toBe(true)
    // value is required
    expect(propertySchema.safeParse({ type: 'string', name: 's' }).success).toBe(false)
  })

  it('PropertyValuesPipe accepts separator-split numeric strings (Joi coerced them)', () => {
    const pipe = new PropertyValuesPipe()
    const body: any = {
      label: 'X',
      properties: [{ type: 'number', name: 'scores', value: '1;2;3', valueSeparator: ';' }]
    }
    expect(() => pipe.transform(body, meta)).not.toThrow()
    expect(() =>
      pipe.transform(
        {
          label: 'X',
          properties: [{ type: 'number', name: 'scores', value: '1;oops;3', valueSeparator: ';' }]
        } as any,
        meta
      )
    ).toThrow(BadRequestException)
  })

  it('searchSchema accepts recursive select expressions and enforces limits', () => {
    const ok = {
      limit: 100,
      where: { name: { $contains: 'a' } },
      select: {
        total: { $count: '*' },
        ratio: { $divide: [{ $sum: '$record.a' }, { $avg: '$record.b', $precision: 2 }] }
      },
      groupBy: ['$record.name']
    }
    expect(searchSchema.safeParse(ok).success).toBe(true)
    expect(searchSchema.safeParse({ limit: 2000 }).success).toBe(false)
    expect(searchSchema.safeParse({ orderBy: 'asc' }).success).toBe(true)
    expect(searchSchema.safeParse({ orderBy: 'sideways' }).success).toBe(false)
  })

  it('createRelationsByKeysSchema allows key-join and many-to-many shapes', () => {
    const keyJoin = {
      source: { label: 'A', key: 'id' },
      target: { label: 'B', key: 'aId' }
    }
    const manyToMany = {
      source: { label: 'A', where: { name: 'x' } },
      target: { label: 'B', where: { name: 'y' } },
      manyToMany: true
    }
    const invalid = { source: { label: 'A' }, target: { label: 'B', key: 'aId' } }
    expect(createRelationsByKeysSchema.safeParse(keyJoin).success).toBe(true)
    expect(createRelationsByKeysSchema.safeParse(manyToMany).success).toBe(true)
    expect(createRelationsByKeysSchema.safeParse(invalid).success).toBe(false)
  })

  it('pipe error message keeps the legacy format', () => {
    try {
      pipeFor(createEmbeddingIndexSchema).transform({ propertyName: 'x' }, meta)
      fail('expected BadRequestException')
    } catch (error: any) {
      expect(error).toBeInstanceOf(BadRequestException)
      expect(error.message).toContain('Request validation of body failed, because:')
      expect(error.message).toContain('label is required and must be a non-empty string')
    }
  })
})
