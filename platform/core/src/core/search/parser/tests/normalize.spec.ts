import { SelectExprMap } from '@/core/common/types'
import { normalizeSelectExpr } from '@/core/search/parser/normalize'

describe('normalizeSelectExpr', () => {
  it('returns null when select is omitted', () => {
    expect(normalizeSelectExpr({})).toBeNull()
  })

  it('passes through a select map as-is', () => {
    const selectMap: SelectExprMap = {
      total: { $sum: '$record.amount' },
      name: '$record.name'
    }
    expect(normalizeSelectExpr({ select: selectMap })).toEqual(selectMap)
  })

  it('passes through complex select expressions', () => {
    const selectMap: SelectExprMap = {
      revenue: { $sum: '$record.amount' },
      orders: { $count: '*' },
      avg: { $avg: '$record.value', $precision: 2 },
      ratio: { $divide: [{ $ref: 'revenue' }, { $ref: 'orders' }] }
    }
    expect(normalizeSelectExpr({ select: selectMap })).toEqual(selectMap)
  })

  it('passes through $collect and $timeBucket expressions', () => {
    const selectMap: SelectExprMap = {
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

  it('returns null when select is empty object', () => {
    expect(normalizeSelectExpr({ select: {} })).toBeNull()
  })
})
