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

  it('identifies isPropertyCriteria correctly 1', () => {
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
})
