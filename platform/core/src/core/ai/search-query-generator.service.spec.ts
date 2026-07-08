// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-nocheck
import { SchemaItem } from '@/core/ai/ai.types'
import { SearchQueryGeneratorService } from '@/core/ai/search-query-generator.service'

describe('SearchQueryGeneratorService.validateAndNormalize', () => {
  const service = new SearchQueryGeneratorService(undefined, undefined)

  const schema: SchemaItem[] = [
    {
      label: 'ACCOUNT',
      count: 10,
      properties: [
        { id: 'p1', name: 'name', type: 'string' },
        { id: 'p2', name: 'balance', type: 'number' }
      ],
      relationships: [{ label: 'ACCOUNT', type: 'TRANSFERRED_TO', direction: 'out' }]
    },
    {
      label: 'BATTLE',
      count: 5,
      properties: [
        { id: 'p3', name: 'name', type: 'string' },
        { id: 'p4', name: 'winner_faction_id', type: 'string' }
      ],
      relationships: []
    }
  ]

  const validate = (query: unknown) => service['validateAndNormalize'](query, schema)

  let selfHosted: string | undefined
  beforeAll(() => {
    selfHosted = process.env.RUSHDB_SELF_HOSTED
    delete process.env.RUSHDB_SELF_HOSTED
  })
  afterAll(() => {
    if (selfHosted !== undefined) {
      process.env.RUSHDB_SELF_HOSTED = selfHosted
    }
  })

  it('accepts the $cycle operator exactly as the spec prompt teaches it', () => {
    const { errors } = validate({
      labels: ['ACCOUNT'],
      where: {
        $cycle: { type: 'TRANSFERRED_TO', direction: 'out', hops: { min: 2, max: 6 } }
      }
    })
    expect(errors).toEqual([])
  })

  it('accepts the $cycle operator nested inside a label block', () => {
    const { errors } = validate({
      labels: ['ACCOUNT'],
      where: {
        ACCOUNT: {
          name: 'x',
          $cycle: { type: 'TRANSFERRED_TO', direction: 'out', hops: { min: 2, max: 6 } }
        }
      }
    })
    expect(errors).toEqual([])
  })

  it('rejects a bare $cycle without a traversal spec', () => {
    const { errors } = validate({
      labels: ['ACCOUNT'],
      where: { $cycle: true }
    })
    expect(errors.some((error) => error.includes("requires a traversal spec with 'hops'"))).toBe(true)
  })

  it('rejects the $cycle operator without hops', () => {
    const { errors } = validate({
      labels: ['ACCOUNT'],
      where: { $cycle: { type: 'TRANSFERRED_TO', direction: 'out' } }
    })
    expect(errors.some((error) => error.includes("requires a traversal spec with 'hops'"))).toBe(true)
  })

  it('rejects $cycle operator hops below the cycle floor of 2', () => {
    const { errors } = validate({
      labels: ['ACCOUNT'],
      where: { $cycle: { type: 'TRANSFERRED_TO', direction: 'out', hops: { min: 1, max: 6 } } }
    })
    expect(errors.some((error) => error.includes("'hops.min'"))).toBe(true)
  })

  it('defaults missing hops.max on the $cycle operator to the deployment cap', () => {
    const { query, errors } = validate({
      labels: ['ACCOUNT'],
      where: { $cycle: { type: 'TRANSFERRED_TO', direction: 'out', hops: { min: 2 } } }
    })
    expect(errors).toEqual([])
    expect((query.where as any).$cycle.hops.max).toBe(10)
  })

  it('rejects the removed block form', () => {
    const { errors } = validate({
      labels: ['ACCOUNT'],
      where: {
        SOME_KEY: {
          $cycle: true,
          $relation: { type: 'TRANSFERRED_TO', direction: 'out', hops: { min: 2, max: 6 } }
        }
      }
    })
    expect(errors.length).toBeGreaterThan(0)
  })

  it('accepts $relation.hops on a regular traversal block', () => {
    const { errors } = validate({
      labels: ['ACCOUNT'],
      where: {
        ACCOUNT: {
          $alias: '$other',
          $relation: { type: 'TRANSFERRED_TO', direction: 'out', hops: { min: 1, max: 4 } }
        }
      }
    })
    expect(errors).toEqual([])
  })

  it('defaults missing hops.max to the deployment cap instead of erroring', () => {
    const { query, warnings, errors } = validate({
      labels: ['ACCOUNT'],
      where: {
        ACCOUNT: { $relation: { type: 'TRANSFERRED_TO', direction: 'out', hops: { min: 1 } } }
      }
    })
    expect(errors).toEqual([])
    expect((query.where as any).ACCOUNT.$relation.hops.max).toBe(10)
    expect(warnings.some((warning) => warning.includes('hops.max'))).toBe(true)
  })

  it('rejects hops without an in/out direction', () => {
    const { errors } = validate({
      labels: ['ACCOUNT'],
      where: {
        ACCOUNT: { $relation: { type: 'TRANSFERRED_TO', direction: 'both', hops: { min: 1, max: 4 } } }
      }
    })
    expect(errors.some((error) => error.includes('must be "in" or "out"'))).toBe(true)
  })

  it('explains how to restructure when $ref is used inside where', () => {
    const { errors } = validate({
      labels: ['ACCOUNT'],
      where: { BATTLE: { winner_faction_id: { $ref: '$record.id' } } }
    })
    expect(
      errors.some(
        (error) =>
          error.includes('Unsupported where operator "$ref"') &&
          error.includes('root on the label that owns that field and use groupBy')
      )
    ).toBe(true)
  })

  it('still flags field references used as plain where values', () => {
    const { errors } = validate({
      labels: ['ACCOUNT'],
      where: { BATTLE: { winner_faction_id: '$record.id' } }
    })
    expect(errors.some((error) => error.includes('field/alias reference'))).toBe(true)
  })

  it('keeps validating an ordinary grouped query without errors', () => {
    const { errors } = validate({
      labels: ['BATTLE'],
      select: { faction: '$record.winner_faction_id', wins: { $count: '*' } },
      groupBy: ['$record.winner_faction_id'],
      orderBy: { wins: 'desc' }
    })
    expect(errors).toEqual([])
  })
})
