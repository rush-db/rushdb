// @ts-nocheck
import { normalizeSelectExpr } from '@/core/search/parser/normalize'
import { BadRequestException } from '@nestjs/common'

describe('normalizeSelectExpr', () => {
  it('returns null when neither select nor aggregate is provided', () => {
    expect(normalizeSelectExpr({})).toBeNull()
    expect(normalizeSelectExpr({ select: {}, aggregate: {} })).toBeNull()
  })

  it('returns null when aggregate is provided without select (falls back to legacy path)', () => {
    const result = normalizeSelectExpr({
      aggregate: { total: { fn: 'sum', field: 'amount', alias: '$record' } }
    })
    expect(result).toBeNull()
  })

  it('passes through a select map as-is', () => {
    const selectMap = {
      total: { $sum: '$record.amount' },
      name: '$record.name'
    }
    expect(normalizeSelectExpr({ select: selectMap })).toEqual(selectMap)
  })

  it('passes through complex select expressions', () => {
    const selectMap = {
      revenue: { $sum: '$record.amount' },
      orders: { $count: '*' },
      avg: { $avg: '$record.value', $precision: 2 },
      ratio: { $divide: [{ $ref: 'revenue' }, { $ref: 'orders' }] }
    }
    expect(normalizeSelectExpr({ select: selectMap })).toEqual(selectMap)
  })

  it('passes through $collect and $timeBucket expressions', () => {
    const selectMap = {
      users: {
        $collect: {
          from: '$user',
          select: { id: '$user.id', name: '$user.name' },
          limit: 10
        }
      },
      bucket: {
        $timeBucket: { field: '$record.createdAt', unit: 'day' }
      }
    }
    expect(normalizeSelectExpr({ select: selectMap })).toEqual(selectMap)
  })

  it('throws when both select and aggregate are provided', () => {
    expect(() =>
      normalizeSelectExpr({
        select: { total: { $sum: '$record.amount' } },
        aggregate: { total: { fn: 'sum', field: 'amount', alias: '$record' } }
      })
    ).toThrow(BadRequestException)
  })

  it('throws with a clear message about mutual exclusivity', () => {
    expect(() =>
      normalizeSelectExpr({
        select: { count: { $count: '*' } },
        aggregate: { count: { fn: 'count' } }
      })
    ).toThrow(/select.*aggregate|aggregate.*select/i)
  })

  it('returns the select map even when aggregate is empty object', () => {
    // Empty aggregate should NOT trigger the "both present" error
    const selectMap = { name: '$record.name' }
    expect(normalizeSelectExpr({ select: selectMap, aggregate: {} })).toEqual(selectMap)
  })

  it('returns null when select is empty object', () => {
    expect(normalizeSelectExpr({ select: {} })).toBeNull()
    expect(normalizeSelectExpr({ select: {}, aggregate: {} })).toBeNull()
  })
})
