import { containsAllowedKeys } from '@/common/utils/containsAllowedKeys'
import { allowedKeys } from '@/core/search/parser/constants'
import { isSubQuery, splitCriteria } from '@/core/search/parser/utils'

describe('splitCriteria', () => {
  it('isSubQuery correctly 1', () => {
    const q1 = {
      $vector: {
        fn: 'cosine',
        value: [1, 2, 3, 4, 5],
        query: {
          $gte: 0.5,
          $lte: 0.8,
          $ne: 0.75
        }
      }
    }

    const result1 = isSubQuery(q1)

    expect(result1).toEqual(false)
  })

  it('isSubQuery correctly 2', () => {
    const q2 = {
      emb: {
        $vector: {
          fn: 'cosine',
          value: [1, 2, 3, 4, 5],
          query: {
            $gte: 0.5,
            $lte: 0.8,
            $ne: 0.75
          }
        }
      }
    }

    const result1 = containsAllowedKeys(q2, allowedKeys)

    expect(result1).toEqual(false)
  })

  it('splitCriteria correctly 1', () => {
    const q1 = {
      emb: {
        $contains: '1',
        $vector: {
          fn: 'cosine',
          value: [1, 2, 3, 4, 5],
          query: {
            $gte: 0.5,
            $lte: 0.8,
            $ne: 0.75
          }
        }
      }
    }

    const result1 = splitCriteria(q1)

    expect(result1).toEqual({
      currentLevel: {
        emb: {
          $contains: '1',
          $vector: {
            fn: 'cosine',
            query: {
              $gte: 0.5,
              $lte: 0.8,
              $ne: 0.75
            },
            value: [1, 2, 3, 4, 5]
          }
        }
      },
      subQueries: {}
    })
  })
})
