// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-nocheck
import { isCurrentLevelCriteria } from '@/core/search/parser/utils'

describe('splitCriteria', () => {
  it('Consider as current level', () => {
    const result = isCurrentLevelCriteria([
      {
        title: 'Forest',
        rating: {
          $or: [{ $and: [{ $gt: 4.5 }, { $lt: 6 }] }, { $ne: 3 }, { $not: { $gte: 4 } }]
        }
      }
    ])
    expect(result).toEqual(true)
  })

  it('Consider as mixed query 1', () => {
    const result = isCurrentLevelCriteria([
      {
        CAR: {
          color: 'red'
        }
      },
      {
        SPOUSE: {
          gender: 'male'
        }
      },
      {
        title: 'Forest'
      }
    ])
    expect(result).toEqual(false)
  })

  it('Consider as mixed query 1', () => {
    const result = isCurrentLevelCriteria({
      CAR: {
        color: 'red'
      },

      SPOUSE: {
        gender: 'male'
      },
      title: 'Forest'
    })
    expect(result).toEqual(false)
  })
})
