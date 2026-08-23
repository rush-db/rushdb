import { RUSHDB_KEY_ID } from '@/core/common/constants'
import { buildOrderByClause, buildSortCriteria } from '@/core/search/parser/orderBy'

describe('orderBy', () => {
  it('sorts by the internal record id descending by default', () => {
    expect(buildSortCriteria(undefined)).toEqual({ [RUSHDB_KEY_ID]: 'desc' })
  })

  it.each(['asc', 'desc'] as const)(
    'maps the public __id alias to the internal record id for %s sorting',
    (direction) => {
      expect(buildSortCriteria({ __id: direction })).toEqual({ [RUSHDB_KEY_ID]: direction })
      expect(buildOrderByClause({ __id: direction })).toEqual([
        `record.\`${RUSHDB_KEY_ID}\` ${direction.toUpperCase()}`
      ])
    }
  )

  it('leaves user properties unchanged when normalizing __id', () => {
    expect(buildSortCriteria({ name: 'asc', __id: 'desc' })).toEqual({
      name: 'asc',
      [RUSHDB_KEY_ID]: 'desc'
    })
  })
})
