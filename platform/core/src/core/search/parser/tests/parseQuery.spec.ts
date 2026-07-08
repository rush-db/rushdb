// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-nocheck
import { parseWhereClause } from '@/core/search/parser/buildQuery'
import { ID_CLAUSE_OPERATOR } from '@/core/search/search.constants'
import { TSearchQueryBuilderOptions } from '@/core/search/search.types'

describe('parseQuery', () => {
  const queryBuilderOptions: TSearchQueryBuilderOptions = {
    nodeAlias: 'record'
  }

  it('parses subQueries correctly 1', () => {
    const result1 = parseWhereClause(
      {
        tag: 'top-sellers',
        AUTHOR: {
          name: 'James Brown',
          $relation: {
            type: 'AUTHORED',
            direction: 'in'
          }
        }
      },
      queryBuilderOptions
    )

    expect(result1).toEqual({
      aliasesMap: {
        $record: 'record'
      },
      nodeAliases: ['record', 'record1'],
      queryParts: {
        record: '(any(value IN record.`tag` WHERE value = "top-sellers"))',
        record1:
          'OPTIONAL MATCH (record)<-[:AUTHORED]-(record1:__RUSHDB__LABEL__RECORD__:`AUTHOR`) WHERE (any(value IN record1.`name` WHERE value = "James Brown"))'
      },
      where: 'record IS NOT NULL AND record1 IS NOT NULL'
    })
  })

  it('parses subQueries correctly 2', () => {
    const result2 = parseWhereClause(
      {
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
        ],

        POST: {
          created: {
            $year: 2011,
            $month: 11,
            $day: 11
          },
          rating: {
            $or: [{ $and: [{ $gt: 4.5 }, { $lt: 6 }] }, { $ne: 3 }, { $not: { $gte: 4 } }]
          },
          title: {
            $ne: 'Forest'
          },
          COMMENT: {
            $relation: { direction: 'out', type: 'COMMENT_TO_POST' },
            authoredBy: {
              $contains: 'Sam'
            }
          }
        }
      },
      queryBuilderOptions
    )
    expect(result2).toEqual({
      aliasesMap: {
        $record: 'record'
      },
      nodeAliases: ['record', 'record1', 'record2'],
      queryParts: {
        record:
          '(((any(value IN record.`name` WHERE value STARTS WITH "Jack") AND any(value IN record.`name` WHERE value ENDS WITH "Rooney"))) OR (any(value IN record.`dateOfBirth` WHERE apoc.convert.fromJsonMap(record.`__RUSHDB__KEY__PROPERTIES__META__`).`dateOfBirth` = "datetime" AND datetime(value) = datetime({year: 1984}))))',
        record1:
          'OPTIONAL MATCH (record)--(record1:__RUSHDB__LABEL__RECORD__:`POST`) WHERE (any(value IN record1.`created` WHERE apoc.convert.fromJsonMap(record1.`__RUSHDB__KEY__PROPERTIES__META__`).`created` = "datetime" AND datetime(value) = datetime({year: 2011, month: 11, day: 11}))) AND (((any(value IN record1.`rating` WHERE value > 4.5) AND any(value IN record1.`rating` WHERE value < 6)) OR any(value IN record1.`rating` WHERE value <> 3) OR (NOT(any(value IN record1.`rating` WHERE value >= 4))))) AND (any(value IN record1.`title` WHERE value <> "Forest"))',
        record2:
          'OPTIONAL MATCH (record1)-[:COMMENT_TO_POST]->(record2:__RUSHDB__LABEL__RECORD__:`COMMENT`) WHERE (any(value IN record2.`authoredBy` WHERE value =~ "(?i).*Sam.*"))'
      },
      where: 'record IS NOT NULL AND (record1 IS NOT NULL AND record2 IS NOT NULL)'
    })
  })

  it('parses subQueries correctly 3', () => {
    const result3 = parseWhereClause(
      {
        created: {
          $year: 2011,
          $month: 11,
          $day: 11
        },
        rating: {
          $or: [{ $and: [{ $gt: 4.5 }, { $lt: 6 }] }, { $ne: 3 }, { $not: { $gte: 4 } }]
        },
        title: {
          $ne: 'Forest'
        },
        COMMENT: {
          $relation: { direction: 'out', type: 'COMMENT_TO_POST' },
          authoredBy: {
            $contains: 'Sam',
            $endsWith: 'Altman'
          },
          POST: {
            title: 'Hey'
          }
        }
      },
      queryBuilderOptions
    )
    expect(result3).toEqual({
      aliasesMap: {
        $record: 'record'
      },
      nodeAliases: ['record', 'record1', 'record2'],
      queryParts: {
        record:
          '((any(value IN record.`created` WHERE apoc.convert.fromJsonMap(record.`__RUSHDB__KEY__PROPERTIES__META__`).`created` = "datetime" AND datetime(value) = datetime({year: 2011, month: 11, day: 11}))) AND (((any(value IN record.`rating` WHERE value > 4.5) AND any(value IN record.`rating` WHERE value < 6)) OR any(value IN record.`rating` WHERE value <> 3) OR (NOT(any(value IN record.`rating` WHERE value >= 4))))) AND (any(value IN record.`title` WHERE value <> "Forest")))',
        record1:
          'OPTIONAL MATCH (record)-[:COMMENT_TO_POST]->(record1:__RUSHDB__LABEL__RECORD__:`COMMENT`) WHERE ((any(value IN record1.`authoredBy` WHERE value =~ "(?i).*Sam.*") AND any(value IN record1.`authoredBy` WHERE value ENDS WITH "Altman")))',
        record2:
          'OPTIONAL MATCH (record1)--(record2:__RUSHDB__LABEL__RECORD__:`POST`) WHERE (any(value IN record2.`title` WHERE value = "Hey"))'
      },
      where: 'record IS NOT NULL AND (record1 IS NOT NULL AND record2 IS NOT NULL)'
    })
  })

  it('parses subQueries correctly 4', () => {
    const result4 = parseWhereClause(
      {
        created: true,
        rating: 5,

        $nor: [
          {
            COMMENT: {
              $relation: { direction: 'out', type: 'COMMENT_TO_POST' },
              authoredBy: {
                $contains: 'Sam',
                $endsWith: 'Altman'
              },
              POST: {
                title: 'Hey'
              }
            }
          },
          {
            title: {
              $ne: 'Forest'
            }
          },
          {
            POST: {
              title: 'Hey',
              COMMENT: {
                $relation: { direction: 'out', type: 'COMMENT_TO_POST' },
                authoredBy: {
                  $contains: 'Sam',
                  $endsWith: 'Altman'
                }
              }
            }
          }
        ]
      },
      queryBuilderOptions
    )
    expect(result4).toEqual({
      aliasesMap: {
        $record: 'record'
      },
      nodeAliases: ['record', 'record1', 'record2', 'record3', 'record4'],
      queryParts: {
        record:
          '((any(value IN record.`created` WHERE value = true)) AND (any(value IN record.`rating` WHERE value = 5)))',
        record1:
          'OPTIONAL MATCH (record)-[:COMMENT_TO_POST]->(record1:__RUSHDB__LABEL__RECORD__:`COMMENT`) WHERE ((any(value IN record1.`authoredBy` WHERE value =~ "(?i).*Sam.*") AND any(value IN record1.`authoredBy` WHERE value ENDS WITH "Altman")))',
        record2:
          'OPTIONAL MATCH (record1)--(record2:__RUSHDB__LABEL__RECORD__:`POST`) WHERE (any(value IN record2.`title` WHERE value = "Hey"))',
        record3:
          'OPTIONAL MATCH (record)--(record3:__RUSHDB__LABEL__RECORD__:`POST`) WHERE (any(value IN record3.`title` WHERE value = "Hey"))',
        record4:
          'OPTIONAL MATCH (record3)-[:COMMENT_TO_POST]->(record4:__RUSHDB__LABEL__RECORD__:`COMMENT`) WHERE ((any(value IN record4.`authoredBy` WHERE value =~ "(?i).*Sam.*") AND any(value IN record4.`authoredBy` WHERE value ENDS WITH "Altman")))'
      },
      where:
        'record IS NOT NULL AND (NOT((record1 IS NOT NULL AND record2 IS NOT NULL) OR (any(value IN record.`title` WHERE value <> "Forest")) OR (record3 IS NOT NULL AND record4 IS NOT NULL)))'
    })
  })

  it('parses subQueries correctly 5', () => {
    const result2 = parseWhereClause(
      {
        COMMENT: {
          authoredBy: {
            $contains: 'Sam',
            $endsWith: 'Altman'
          },
          POST: {
            title: 'Hey'
          }
        }
      },
      queryBuilderOptions
    )
    expect(result2).toEqual({
      aliasesMap: {
        $record: 'record'
      },
      nodeAliases: ['record', 'record1', 'record2'],
      queryParts: {
        record: '',
        record1:
          'OPTIONAL MATCH (record)--(record1:__RUSHDB__LABEL__RECORD__:`COMMENT`) WHERE ((any(value IN record1.`authoredBy` WHERE value =~ "(?i).*Sam.*") AND any(value IN record1.`authoredBy` WHERE value ENDS WITH "Altman")))',
        record2:
          'OPTIONAL MATCH (record1)--(record2:__RUSHDB__LABEL__RECORD__:`POST`) WHERE (any(value IN record2.`title` WHERE value = "Hey"))'
      },
      where: 'record IS NOT NULL AND (record1 IS NOT NULL AND record2 IS NOT NULL)'
    })
  })

  it('parses subQueries correctly 6', () => {
    const result6 = parseWhereClause(
      {
        created: true,
        rating: 5,

        $or: [
          {
            CAR: {
              color: 'red'
            }
          },
          {
            title: {
              $ne: 'Forest'
            }
          },
          {
            SPOUSE: {
              gender: 'male'
            }
          }
        ]
      },
      queryBuilderOptions
    )
    expect(result6).toEqual({
      aliasesMap: {
        $record: 'record'
      },
      nodeAliases: ['record', 'record1', 'record2'],
      queryParts: {
        record:
          '((any(value IN record.`created` WHERE value = true)) AND (any(value IN record.`rating` WHERE value = 5)))',
        record1:
          'OPTIONAL MATCH (record)--(record1:__RUSHDB__LABEL__RECORD__:`CAR`) WHERE (any(value IN record1.`color` WHERE value = "red"))',
        record2:
          'OPTIONAL MATCH (record)--(record2:__RUSHDB__LABEL__RECORD__:`SPOUSE`) WHERE (any(value IN record2.`gender` WHERE value = "male"))'
      },
      where:
        'record IS NOT NULL AND (record1 IS NOT NULL OR record2 IS NOT NULL OR (any(value IN record.`title` WHERE value <> "Forest")))'
    })
  })

  it('parses subQueries correctly 7', () => {
    const result7 = parseWhereClause(
      {
        created: true,
        rating: 5,

        $or: [
          {
            CAR: {
              color: 'red',
              $xor: [
                {
                  SPOUSE: {
                    gender: 'male'
                  }
                },
                {
                  title: {
                    $ne: 'Forest'
                  }
                }
              ]
            }
          }
        ]
      },
      queryBuilderOptions
    )
    expect(result7).toEqual({
      aliasesMap: {
        $record: 'record'
      },
      nodeAliases: ['record', 'record1', 'record2'],
      queryParts: {
        record:
          '((any(value IN record.`created` WHERE value = true)) AND (any(value IN record.`rating` WHERE value = 5)))',
        record1:
          'OPTIONAL MATCH (record)--(record1:__RUSHDB__LABEL__RECORD__:`CAR`) WHERE (any(value IN record1.`color` WHERE value = "red"))',
        record2:
          'OPTIONAL MATCH (record1)--(record2:__RUSHDB__LABEL__RECORD__:`SPOUSE`) WHERE (any(value IN record2.`gender` WHERE value = "male"))'
      },
      where:
        'record IS NOT NULL AND ((record1 IS NOT NULL AND (record2 IS NOT NULL XOR (any(value IN record1.`title` WHERE value <> "Forest")))))'
    })
  })

  it('parses subQueries correctly 8', () => {
    const result8 = parseWhereClause(
      {
        created: true,
        rating: 5,

        $or: {
          CAR: {
            color: 'red',
            $xor: {
              SPOUSE: {
                gender: 'male'
              },
              title: {
                $ne: 'Forest'
              }
            }
          }
        }
      },
      queryBuilderOptions
    )
    expect(result8).toEqual({
      aliasesMap: {
        $record: 'record'
      },
      nodeAliases: ['record', 'record1', 'record2'],
      queryParts: {
        record:
          '((any(value IN record.`created` WHERE value = true)) AND (any(value IN record.`rating` WHERE value = 5)))',
        record1:
          'OPTIONAL MATCH (record)--(record1:__RUSHDB__LABEL__RECORD__:`CAR`) WHERE (any(value IN record1.`color` WHERE value = "red"))',
        record2:
          'OPTIONAL MATCH (record1)--(record2:__RUSHDB__LABEL__RECORD__:`SPOUSE`) WHERE (any(value IN record2.`gender` WHERE value = "male"))'
      },
      where:
        'record IS NOT NULL AND ((record1 IS NOT NULL AND (record2 IS NOT NULL XOR (any(value IN record1.`title` WHERE value <> "Forest")))))'
    })
  })

  it('parses subQueries correctly 9', () => {
    const result9 = parseWhereClause(
      {
        $or: {
          CAR: {
            color: 'red'
          },
          SPOUSE: {
            gender: 'male'
          }
        }
      },
      queryBuilderOptions
    )
    expect(result9).toEqual({
      aliasesMap: {
        $record: 'record'
      },
      nodeAliases: ['record', 'record1', 'record2'],
      queryParts: {
        record: '',
        record1:
          'OPTIONAL MATCH (record)--(record1:__RUSHDB__LABEL__RECORD__:`CAR`) WHERE (any(value IN record1.`color` WHERE value = "red"))',
        record2:
          'OPTIONAL MATCH (record)--(record2:__RUSHDB__LABEL__RECORD__:`SPOUSE`) WHERE (any(value IN record2.`gender` WHERE value = "male"))'
      },
      where: 'record IS NOT NULL AND (record1 IS NOT NULL OR record2 IS NOT NULL)'
    })
  })

  it('parses ID criteria correctly 10', () => {
    const result10 = parseWhereClause(
      {
        [ID_CLAUSE_OPERATOR]: '123'
      },
      queryBuilderOptions
    )
    expect(result10).toEqual({
      aliasesMap: {
        $record: 'record'
      },
      nodeAliases: ['record'],
      queryParts: {
        record: '(any(value IN record.`__RUSHDB__KEY__ID__` WHERE value = "123"))'
      },
      where: ''
    })
  })

  it('parses nested ID criteria correctly 11', () => {
    const result10 = parseWhereClause(
      {
        [ID_CLAUSE_OPERATOR]: '123',
        CAR: {
          [ID_CLAUSE_OPERATOR]: '567'
        }
      },
      queryBuilderOptions
    )
    expect(result10).toEqual({
      aliasesMap: {
        $record: 'record'
      },
      nodeAliases: ['record', 'record1'],
      queryParts: {
        record: '(any(value IN record.`__RUSHDB__KEY__ID__` WHERE value = "123"))',
        record1:
          'OPTIONAL MATCH (record)--(record1:__RUSHDB__LABEL__RECORD__:`CAR`) WHERE (any(value IN record1.`__RUSHDB__KEY__ID__` WHERE value = "567"))'
      },
      where: 'record IS NOT NULL AND record1 IS NOT NULL'
    })
  })

  it('parses empty where correctly 12', () => {
    const result10 = parseWhereClause({}, queryBuilderOptions)
    expect(result10).toEqual({
      nodeAliases: ['record'],
      aliasesMap: { $record: 'record' },
      queryParts: {
        record: ''
      },
      where: ''
    })
  })

  it('parses empty nested criteria correctly 13', () => {
    const result10 = parseWhereClause(
      {
        $or: {
          CAR: {
            color: 'red'
          },
          SPOUSE: {
            gender: 'male',
            $not: {
              CAR: {}
            }
          }
        }
      },
      queryBuilderOptions
    )
    expect(result10).toEqual({
      aliasesMap: {
        $record: 'record'
      },
      nodeAliases: ['record', 'record1', 'record2', 'record3'],
      queryParts: {
        record: '',
        record1:
          'OPTIONAL MATCH (record)--(record1:__RUSHDB__LABEL__RECORD__:`CAR`) WHERE (any(value IN record1.`color` WHERE value = "red"))',
        record2:
          'OPTIONAL MATCH (record)--(record2:__RUSHDB__LABEL__RECORD__:`SPOUSE`) WHERE (any(value IN record2.`gender` WHERE value = "male"))',
        record3: 'OPTIONAL MATCH (record2)--(record3:__RUSHDB__LABEL__RECORD__:`CAR`)'
      },
      where:
        'record IS NOT NULL AND (record1 IS NOT NULL OR (record2 IS NOT NULL AND (NOT(record3 IS NOT NULL))))'
    })
  })

  it('parses logical grouping on top level correctly 14', () => {
    const result10 = parseWhereClause(
      {
        $xor: [{ foundingDate: { $gte: '1989-10-01T19:05:17.780Z' } }, { city: 'Doral' }]
      },
      queryBuilderOptions
    )
    expect(result10).toEqual({
      aliasesMap: {
        $record: 'record'
      },
      nodeAliases: ['record'],
      queryParts: {
        record:
          '((any(value IN record.`foundingDate` WHERE apoc.convert.fromJsonMap(record.`__RUSHDB__KEY__PROPERTIES__META__`).`foundingDate` = "datetime" AND datetime(value) >= datetime("1989-10-01T19:05:17.780Z"))) XOR (any(value IN record.`city` WHERE value = "Doral")))'
      },
      where: ''
    })
  })
})

describe('variable-length traversal ($relation.hops)', () => {
  const options = (): TSearchQueryBuilderOptions => ({ nodeAlias: 'record' })

  it('renders exact hop count for typed directed relation', () => {
    const result = parseWhereClause(
      {
        MANAGER: {
          $relation: { type: 'REPORTS_TO', direction: 'out', hops: 3 }
        }
      },
      options()
    )

    expect(result).toEqual({
      aliasesMap: { $record: 'record' },
      nodeAliases: ['record', 'record1'],
      queryParts: {
        record: '',
        record1: 'OPTIONAL MATCH (record)-[:REPORTS_TO*3]->(record1:__RUSHDB__LABEL__RECORD__:`MANAGER`)'
      },
      where: 'record IS NOT NULL AND record1 IS NOT NULL'
    })
  })

  it('defaults min to 1 and renders range for typed incoming relation', () => {
    const result = parseWhereClause(
      {
        AUTHOR: {
          $relation: { type: 'AUTHORED', direction: 'in', hops: { max: 5 } }
        }
      },
      options()
    )

    expect(result.queryParts.record1).toEqual(
      'OPTIONAL MATCH (record)<-[:AUTHORED*1..5]-(record1:__RUSHDB__LABEL__RECORD__:`AUTHOR`)'
    )
  })

  it('renders explicit min..max range without direction', () => {
    const result = parseWhereClause(
      {
        CATEGORY: {
          $relation: { type: 'HAS_PARENT', hops: { min: 2, max: 5 } }
        }
      },
      options()
    )

    expect(result.queryParts.record1).toEqual(
      'OPTIONAL MATCH (record)-[:HAS_PARENT*2..5]-(record1:__RUSHDB__LABEL__RECORD__:`CATEGORY`)'
    )
  })

  it('binds rel var and excludes VALUE edges for untyped hops with criteria', () => {
    const result = parseWhereClause(
      {
        CATEGORY: {
          $relation: { hops: { max: 3 } },
          name: 'Electronics'
        }
      },
      options()
    )

    expect(result).toEqual({
      aliasesMap: { $record: 'record' },
      nodeAliases: ['record', 'record1'],
      queryParts: {
        record: '',
        record1:
          'OPTIONAL MATCH (record)-[rels1*1..3]-(record1:__RUSHDB__LABEL__RECORD__:`CATEGORY`) WHERE all(r IN rels1 WHERE type(r) <> \'__RUSHDB__RELATION__VALUE__\') AND (any(value IN record1.`name` WHERE value = "Electronics"))'
      },
      where: 'record IS NOT NULL AND record1 IS NOT NULL'
    })
  })

  it('emits the VALUE-edge filter alone for an untyped hops block without criteria', () => {
    const result = parseWhereClause(
      {
        CATEGORY: {
          $relation: { direction: 'out', hops: { max: 4 } }
        }
      },
      options()
    )

    expect(result.queryParts.record1).toEqual(
      "OPTIONAL MATCH (record)-[rels1*1..4]->(record1:__RUSHDB__LABEL__RECORD__:`CATEGORY`) WHERE all(r IN rels1 WHERE type(r) <> '__RUSHDB__RELATION__VALUE__')"
    )
    expect(result.nodeAliases).toEqual(['record', 'record1'])
  })

  it('registers $alias for a hops endpoint', () => {
    const result = parseWhereClause(
      {
        EMPLOYEE: {
          $alias: '$subordinate',
          $relation: { type: 'MANAGES', direction: 'out', hops: { max: 4 } }
        }
      },
      options()
    )

    expect(result.aliasesMap).toEqual({ $record: 'record', $subordinate: 'record1' })
  })

  it('anchors nested labels beneath a hops endpoint', () => {
    const result = parseWhereClause(
      {
        EMPLOYEE: {
          $relation: { type: 'MANAGES', direction: 'out', hops: { max: 4 } },
          PROJECT: {
            title: 'Apollo'
          }
        }
      },
      options()
    )

    expect(result.queryParts).toEqual({
      record: '',
      record1: 'OPTIONAL MATCH (record)-[:MANAGES*1..4]->(record1:__RUSHDB__LABEL__RECORD__:`EMPLOYEE`)',
      record2:
        'OPTIONAL MATCH (record1)--(record2:__RUSHDB__LABEL__RECORD__:`PROJECT`) WHERE (any(value IN record2.`title` WHERE value = "Apollo"))'
    })
    expect(result.where).toEqual('record IS NOT NULL AND (record1 IS NOT NULL AND record2 IS NOT NULL)')
  })

  it('allows unbounded max when the cap is unlimited', () => {
    const result = parseWhereClause(
      {
        CATEGORY: {
          $relation: { type: 'HAS_PARENT', direction: 'out', hops: { min: 2 } }
        }
      },
      { nodeAlias: 'record', maxHops: Infinity }
    )

    expect(result.queryParts.record1).toEqual(
      'OPTIONAL MATCH (record)-[:HAS_PARENT*2..]->(record1:__RUSHDB__LABEL__RECORD__:`CATEGORY`)'
    )
  })

  it.each([
    ['zero', 0],
    ['negative', -1],
    ['float', 2.5],
    ['string', '3']
  ])('rejects %s hops', (_, hops) => {
    expect(() => parseWhereClause({ MANAGER: { $relation: { type: 'T', hops } } }, options())).toThrow()
  })

  it('rejects min greater than max', () => {
    expect(() =>
      parseWhereClause({ MANAGER: { $relation: { type: 'T', hops: { min: 5, max: 2 } } } }, options())
    ).toThrow("'hops.min'")
  })

  it('rejects max beyond the effective cap', () => {
    expect(() =>
      parseWhereClause(
        { MANAGER: { $relation: { type: 'T', hops: { max: 11 } } } },
        { nodeAlias: 'record', maxHops: 10 }
      )
    ).toThrow("'hops.max' must be an integer between 1 and 10")
  })

  it('rejects unbounded max when the cap is finite', () => {
    expect(() =>
      parseWhereClause(
        { MANAGER: { $relation: { type: 'T', hops: { min: 2 } } } },
        { nodeAlias: 'record', maxHops: 10 }
      )
    ).toThrow('unbounded traversal is not allowed')
  })

  it('rejects unrecognized direction when hops is set', () => {
    expect(() =>
      parseWhereClause(
        { MANAGER: { $relation: { type: 'T', direction: 'sideways', hops: { max: 3 } } } },
        options()
      )
    ).toThrow("'direction'")
  })
})

// { $cycle: { type?, direction, hops } } at record level — the value IS the traversal
// spec. A cycle has no endpoint, so there is no key, no $relation wrapper, no $alias.
describe('cycle detection ($cycle)', () => {
  const options = (): TSearchQueryBuilderOptions => ({ nodeAlias: 'record' })

  it('compiles a typed cycle to an EXISTS predicate anchored to the parent', () => {
    const result = parseWhereClause(
      { $cycle: { type: 'TRANSFERRED_TO', direction: 'out', hops: { max: 6 } } },
      options()
    )

    // No OPTIONAL MATCH part and no bound rel var: EXISTS short-circuits at the first
    // cycle found per record instead of enumerating every path and null-checking after.
    expect(result).toEqual({
      aliasesMap: { $record: 'record' },
      nodeAliases: ['record'],
      queryParts: {
        record: ''
      },
      where: 'record IS NOT NULL AND EXISTS { MATCH (record)-[:TRANSFERRED_TO*2..6]->(record) }'
    })
  })

  it('adds the VALUE-edge filter inside EXISTS for an untyped cycle', () => {
    const result = parseWhereClause({ $cycle: { direction: 'out', hops: { max: 6 } } }, options())

    expect(result.where).toEqual(
      'record IS NOT NULL AND ' +
        "EXISTS { MATCH (record)-[rels1*2..6]->(record) WHERE all(r IN rels1 WHERE type(r) <> '__RUSHDB__RELATION__VALUE__') }"
    )
    expect(result.queryParts).toEqual({ record: '' })
  })

  it('keeps level numbering for siblings after a cycle predicate', () => {
    const result = parseWhereClause(
      {
        $cycle: { type: 'T', direction: 'out', hops: { max: 3 } },
        FRIEND: {
          name: 'Bob'
        }
      },
      options()
    )

    expect(result.queryParts).toEqual({
      record: '',
      record2:
        'OPTIONAL MATCH (record)--(record2:__RUSHDB__LABEL__RECORD__:`FRIEND`) WHERE (any(value IN record2.`name` WHERE value = "Bob"))'
    })
    expect(result.nodeAliases).toEqual(['record', 'record2'])
    expect(result.where).toEqual(
      'record IS NOT NULL AND EXISTS { MATCH (record)-[:T*2..3]->(record) } AND record2 IS NOT NULL'
    )
  })

  it('defaults cycle min to 2 and rejects hops below it', () => {
    expect(() => parseWhereClause({ $cycle: { type: 'T', hops: 1 } }, options())).toThrow()
    expect(() => parseWhereClause({ $cycle: { type: 'T', hops: { min: 1, max: 3 } } }, options())).toThrow(
      "'hops.min'"
    )
  })

  it('anchors to a related record when nested in a label block', () => {
    const result = parseWhereClause(
      {
        ACCOUNT: {
          country: 'US',
          $cycle: { direction: 'out', hops: { max: 4 } }
        }
      },
      options()
    )

    expect(result.where).toEqual(
      'record IS NOT NULL AND (record1 IS NOT NULL AND ' +
        "EXISTS { MATCH (record1)-[rels2*2..4]->(record1) WHERE all(r IN rels2 WHERE type(r) <> '__RUSHDB__RELATION__VALUE__') })"
    )
  })

  it('composes under $or with other blocks', () => {
    const result = parseWhereClause(
      {
        $or: {
          $cycle: { type: 'T', direction: 'out', hops: { max: 3 } },
          EMPLOYEE: { name: 'X' }
        }
      },
      options()
    )

    expect(result.where).toEqual(
      'record IS NOT NULL AND (EXISTS { MATCH (record)-[:T*2..3]->(record) } OR record2 IS NOT NULL)'
    )
  })

  it('negates with $not (acyclic check)', () => {
    const result = parseWhereClause(
      { $not: { $cycle: { type: 'T', direction: 'out', hops: { max: 3 } } } },
      options()
    )

    expect(result.where).toEqual(
      'record IS NOT NULL AND (NOT(EXISTS { MATCH (record)-[:T*2..3]->(record) }))'
    )
  })

  it('supports several cycle predicates at one level via an $and array', () => {
    const result = parseWhereClause(
      {
        $and: [
          { $cycle: { type: 'A', direction: 'out', hops: { max: 3 } } },
          { $cycle: { type: 'B', direction: 'out', hops: { max: 4 } } }
        ]
      },
      options()
    )

    expect(result.where).toContain('EXISTS { MATCH (record)-[:A*2..3]->(record) }')
    expect(result.where).toContain('EXISTS { MATCH (record)-[:B*2..4]->(record) }')
  })

  it.each([
    ['a bare boolean', { $cycle: true }],
    ['a string value', { $cycle: 'T' }],
    ['a spec without hops', { $cycle: { type: 'T' } }]
  ])('rejects $cycle with %s', (_, where) => {
    expect(() => parseWhereClause(where as never, options())).toThrow("'$cycle' requires")
  })

  it('rejects the removed block form with operator-form guidance', () => {
    expect(() =>
      parseWhereClause(
        { SOME_KEY: { $cycle: true, $relation: { type: 'T', direction: 'out', hops: { max: 3 } } } },
        options()
      )
    ).toThrow("'$cycle' requires")
  })
})
