import { isPropertyCriteria } from '@/core/search/parser/utils'

describe('isPropertyCriteria', () => {
  it('identifies isPropertyCriteria correctly 1', () => {
    const result1 = isPropertyCriteria({
      $or: [
        {
          name: {
            $startsWith: 'Jack',
            $endsWith: 'Rooney'
          }
        },
        {
          dateOfBirth: {
            $year: 1984
          }
        }
      ]
    })

    expect(result1).toEqual(false)
  })

  it('identifies isPropertyCriteria correctly 2', () => {
    const result1 = isPropertyCriteria({
      $or: [
        {
          $startsWith: 'Jack',
          $endsWith: 'Rooney'
        },
        {
          $year: 1984
        }
      ]
    })

    expect(result1).toEqual(true)
  })

  it('identifies isPropertyCriteria correctly 3', () => {
    const result1 = isPropertyCriteria({
      $vector: {
        fn: 'cosine',
        query: [1, 2, 3, 4, 5],
        threshold: {
          $gte: 0.5,
          $lte: 0.8,
          $ne: 0.75
        }
      }
    })

    expect(result1).toEqual(true)
  })
})
