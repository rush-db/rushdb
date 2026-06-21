// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-nocheck
import { BadRequestException } from '@nestjs/common'

import { compileExpr, compileSelectMap, isSimpleFieldRef } from '@/core/search/parser/compileExpr'

const aliasesMap = {
  $record: 'record',
  $employee: 'record1',
  $department: 'record2'
}

// ── isSimpleFieldRef ─────────────────────────────────────────────────────────

describe('isSimpleFieldRef', () => {
  it('returns true for $-prefixed strings', () => {
    expect(isSimpleFieldRef('$record.name')).toBe(true)
    expect(isSimpleFieldRef('$employee.salary')).toBe(true)
  })

  it('returns false for non-string expressions', () => {
    expect(isSimpleFieldRef({ $sum: '$record.amount' })).toBe(false)
    expect(isSimpleFieldRef(42)).toBe(false)
    expect(isSimpleFieldRef(true)).toBe(false)
  })

  it('returns false for plain strings not starting with $', () => {
    expect(isSimpleFieldRef('name')).toBe(false)
    expect(isSimpleFieldRef('*')).toBe(false)
  })
})

// ── compileExpr ──────────────────────────────────────────────────────────────

describe('compileExpr', () => {
  const selectMap = {}

  it('compiles a field reference', () => {
    expect(compileExpr('$record.amount', aliasesMap, selectMap)).toBe('record.`amount`')
    expect(compileExpr('$employee.salary', aliasesMap, selectMap)).toBe('record1.`salary`')
  })

  it('resolves __id alias to internal key', () => {
    expect(compileExpr('$record.__id', aliasesMap, selectMap)).toBe('record.`__RUSHDB__KEY__ID__`')
  })

  it('compiles alias-only reference (no field)', () => {
    expect(compileExpr('$employee', aliasesMap, selectMap)).toBe('record1')
  })

  it('compiles literal number', () => {
    expect(compileExpr(42, aliasesMap, selectMap)).toBe('42')
    expect(compileExpr(0, aliasesMap, selectMap)).toBe('0')
  })

  it('compiles literal boolean', () => {
    expect(compileExpr(true, aliasesMap, selectMap)).toBe('true')
    expect(compileExpr(false, aliasesMap, selectMap)).toBe('false')
  })

  it('compiles $ref', () => {
    const map = { revenue: { $sum: '$record.amount' } }
    expect(compileExpr({ $ref: 'revenue' }, aliasesMap, map)).toBe('`revenue`')
  })

  it('throws for $ref pointing to unknown key', () => {
    expect(() => compileExpr({ $ref: 'nonExistent' }, aliasesMap, {})).toThrow(BadRequestException)
  })

  it('compiles $sum', () => {
    expect(compileExpr({ $sum: '$record.amount' }, aliasesMap, selectMap)).toBe('sum(record.`amount`)')
  })

  it('compiles $avg without precision', () => {
    expect(compileExpr({ $avg: '$record.value' }, aliasesMap, selectMap)).toBe('avg(record.`value`)')
  })

  it('compiles $avg with precision > 0', () => {
    expect(compileExpr({ $avg: '$record.value', $precision: 2 }, aliasesMap, selectMap)).toBe(
      'round(avg(record.`value`), 2)'
    )
  })

  it('compiles $avg with precision = 0 (toInteger)', () => {
    expect(compileExpr({ $avg: '$record.value', $precision: 0 }, aliasesMap, selectMap)).toBe(
      'toInteger(avg(record.`value`))'
    )
  })

  it('compiles $count with *', () => {
    expect(compileExpr({ $count: '*' }, aliasesMap, selectMap)).toBe('count(DISTINCT record)')
  })

  it('compiles $count with a field ref', () => {
    expect(compileExpr({ $count: '$record.id' }, aliasesMap, selectMap)).toBe('count(DISTINCT record.`id`)')
  })

  it('compiles $count with alias-only (count nodes)', () => {
    expect(compileExpr({ $count: '$employee' }, aliasesMap, selectMap)).toBe('count(DISTINCT record1)')
  })

  it('compiles $min and $max', () => {
    expect(compileExpr({ $min: '$record.price' }, aliasesMap, selectMap)).toBe('min(record.`price`)')
    expect(compileExpr({ $max: '$record.price' }, aliasesMap, selectMap)).toBe('max(record.`price`)')
  })

  it('compiles $divide', () => {
    expect(compileExpr({ $divide: ['$record.a', '$record.b'] }, aliasesMap, selectMap)).toBe(
      '(record.`a` / record.`b`)'
    )
  })

  it('compiles $multiply', () => {
    expect(compileExpr({ $multiply: ['$record.price', '$record.qty'] }, aliasesMap, selectMap)).toBe(
      '(record.`price` * record.`qty`)'
    )
  })

  it('compiles $add and $subtract', () => {
    expect(compileExpr({ $add: ['$record.a', '$record.b'] }, aliasesMap, selectMap)).toBe(
      '(record.`a` + record.`b`)'
    )
    expect(compileExpr({ $subtract: ['$record.a', '$record.b'] }, aliasesMap, selectMap)).toBe(
      '(record.`a` - record.`b`)'
    )
  })

  it('compiles nested math inside $sum', () => {
    expect(
      compileExpr({ $sum: { $multiply: ['$record.price', '$record.qty'] } }, aliasesMap, selectMap)
    ).toBe('sum((record.`price` * record.`qty`))')
  })

  it('compiles $subtract with $ref operands', () => {
    const map = {
      revenue: { $sum: '$record.amount' },
      cost: { $sum: '$record.cost' }
    }
    expect(compileExpr({ $subtract: [{ $ref: 'revenue' }, { $ref: 'cost' }] }, aliasesMap, map)).toBe(
      '(`revenue` - `cost`)'
    )
  })

  it('throws for unknown alias in field ref', () => {
    expect(() => compileExpr('$unknown.field', aliasesMap, selectMap)).toThrow(BadRequestException)
  })

  it('throws for unknown expression operator', () => {
    expect(() => compileExpr({ $unknownOp: 42 } as any, aliasesMap, selectMap)).toThrow(BadRequestException)
  })
})

// ── compileSelectMap ─────────────────────────────────────────────────────────

describe('compileSelectMap', () => {
  it('compiles a simple field ref — no WITH, field in RETURN projection', () => {
    const { withPart, returnPart } = compileSelectMap({ projectName: '$record.name' }, aliasesMap)
    expect(withPart).toBe('')
    expect(returnPart).toContain('`projectName`: record.`name`')
    expect(returnPart).toContain('.*')
  })

  it('compiles $sum — produces WITH + RETURN alias ref', () => {
    const { withPart, returnPart } = compileSelectMap({ total: { $sum: '$record.amount' } }, aliasesMap)
    expect(withPart).toContain('sum(record.`amount`) AS `total`')
    expect(returnPart).toContain('`total`')
  })

  it('compiles $count with *', () => {
    const { withPart } = compileSelectMap({ count: { $count: '*' } }, aliasesMap)
    expect(withPart).toContain('count(DISTINCT record) AS `count`')
  })

  it('compiles mixed field refs and aggregations', () => {
    const { withPart, returnPart } = compileSelectMap(
      {
        projectName: '$record.name',
        headcount: { $count: '$employee' }
      },
      aliasesMap
    )
    expect(withPart).toContain('count(DISTINCT record1) AS `headcount`')
    expect(returnPart).toContain('`projectName`: record.`name`')
    expect(returnPart).toContain('`headcount`')
  })

  it('correctly orders $ref dependencies across two WITH layers', () => {
    const { withPart, returnPart } = compileSelectMap(
      {
        revenue: { $sum: '$record.amount' },
        orders: { $count: '*' },
        avgOrderValue: { $divide: [{ $ref: 'revenue' }, { $ref: 'orders' }] }
      },
      aliasesMap
    )
    // Two WITH layers expected
    const withLines = withPart.split('\n').filter(Boolean)
    expect(withLines.length).toBe(2)
    // Layer 1: revenue and orders
    expect(withLines[0]).toContain('sum(record.`amount`) AS `revenue`')
    expect(withLines[0]).toContain('count(DISTINCT record) AS `orders`')
    // Layer 2: avgOrderValue referencing layer 1
    expect(withLines[1]).toContain('(`revenue` / `orders`) AS `avgOrderValue`')
    expect(withLines[1]).toContain('`revenue`')
    expect(withLines[1]).toContain('`orders`')
    // All keys in RETURN
    expect(returnPart).toContain('`revenue`')
    expect(returnPart).toContain('`orders`')
    expect(returnPart).toContain('`avgOrderValue`')
  })

  it('throws on circular $ref', () => {
    expect(() =>
      compileSelectMap(
        {
          a: { $divide: [{ $ref: 'b' }, 1] },
          b: { $divide: [{ $ref: 'a' }, 1] }
        },
        aliasesMap
      )
    ).toThrow(BadRequestException)
  })

  it('compiles $avg with $precision', () => {
    const { withPart } = compileSelectMap({ avg: { $avg: '$record.value', $precision: 2 } }, aliasesMap)
    expect(withPart).toContain('round(avg(record.`value`), 2) AS `avg`')
  })

  it('returns default projection when no aggregations (only field refs)', () => {
    const { withPart, returnPart } = compileSelectMap(
      { name: '$record.name', budget: '$record.budget' },
      aliasesMap
    )
    expect(withPart).toBe('')
    expect(returnPart).toContain('`name`: record.`name`')
    expect(returnPart).toContain('`budget`: record.`budget`')
    expect(returnPart).toContain('.*')
  })

  it('handles empty selectMap — returns default projection', () => {
    const { withPart, returnPart } = compileSelectMap({}, aliasesMap)
    expect(withPart).toBe('')
    expect(returnPart).toContain('.*')
  })

  it('rejects alias-only groupBy values', () => {
    expect(() => compileSelectMap({ count: { $count: '*' } }, aliasesMap, ['$record'])).toThrow(
      /groupBy field reference "\$record" must include a property name/
    )
  })
})
