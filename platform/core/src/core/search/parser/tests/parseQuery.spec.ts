// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-nocheck
import { RUSHDB_KEY_ID } from '@/core/common/constants'
import { parse } from '@/core/search/parser/parse'
import { ID_CLAUSE_OPERATOR } from '@/core/search/search.constants'
import { TSearchQueryBuilderOptions } from '@/core/search/search.types'

describe('parseQuery', () => {
  const queryBuilderOptions: TSearchQueryBuilderOptions = {
    nodeAlias: 'record'
  }

  it('parses subQueries correctly 1', () => {
    const result1 = parse(
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
      nodeAliases: ['record', 'record1'],
      aliasesMap: { $record: 'record' },
      queryParts: {
        record: '(any(value IN record.tag WHERE value = "top-sellers"))',
        record1:
          'OPTIONAL MATCH (record)<-[:AUTHORED]-(record1:AUTHOR) WHERE (any(value IN record1.name WHERE value = "James Brown"))'
      },
      where: 'record IS NOT NULL AND record1 IS NOT NULL'
    })
  })

  it('parses subQueries correctly 2', () => {
    const result2 = parse(
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
      nodeAliases: ['record', 'record1', 'record2'],
      aliasesMap: { $record: 'record' },
      queryParts: {
        record:
          '((any(value IN record.name WHERE value STARTS WITH "Jack") AND any(value IN record.name WHERE value ENDS WITH "Rooney")) OR (any(value IN record.dateOfBirth WHERE apoc.convert.fromJsonMap(`record`.`__RUSHDB__KEY__PROPERTIES__META__`).`dateOfBirth` = "datetime" AND datetime(value) = datetime({year: 1984}))))',
        record1:
          'OPTIONAL MATCH (record)--(record1:POST) WHERE (any(value IN record1.created WHERE apoc.convert.fromJsonMap(`record1`.`__RUSHDB__KEY__PROPERTIES__META__`).`created` = "datetime" AND datetime(value) = datetime({year: 2011, month: 11, day: 11}))) AND (((any(value IN record1.rating WHERE value > 4.5) AND any(value IN record1.rating WHERE value < 6)) OR any(value IN record1.rating WHERE value <> 3) OR (NOT(any(value IN record1.rating WHERE value >= 4))))) AND (any(value IN record1.title WHERE value <> "Forest"))',
        record2: `OPTIONAL MATCH (record1)-[:COMMENT_TO_POST]->(record2:COMMENT) WHERE (any(value IN record2.authoredBy WHERE value =~ "(?i).*Sam.*"))`
      },
      where: 'record IS NOT NULL AND (record1 IS NOT NULL AND record2 IS NOT NULL)'
    })
  })

  it('parses subQueries correctly 3', () => {
    const result3 = parse(
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
      nodeAliases: ['record', 'record1', 'record2'],
      aliasesMap: { $record: 'record' },
      queryParts: {
        record:
          '(any(value IN record.created WHERE apoc.convert.fromJsonMap(`record`.`__RUSHDB__KEY__PROPERTIES__META__`).`created` = "datetime" AND datetime(value) = datetime({year: 2011, month: 11, day: 11}))) AND (((any(value IN record.rating WHERE value > 4.5) AND any(value IN record.rating WHERE value < 6)) OR any(value IN record.rating WHERE value <> 3) OR (NOT(any(value IN record.rating WHERE value >= 4))))) AND (any(value IN record.title WHERE value <> "Forest"))',
        record1:
          'OPTIONAL MATCH (record)-[:COMMENT_TO_POST]->(record1:COMMENT) WHERE (any(value IN record1.authoredBy WHERE value =~ "(?i).*Sam.*") AND any(value IN record1.authoredBy WHERE value ENDS WITH "Altman"))',
        record2:
          'OPTIONAL MATCH (record1)--(record2:POST) WHERE (any(value IN record2.title WHERE value = "Hey"))'
      },
      where: 'record IS NOT NULL AND (record1 IS NOT NULL AND record2 IS NOT NULL)'
    })
  })

  it('parses subQueries correctly 4', () => {
    const result4 = parse(
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
      nodeAliases: ['record', 'record1', 'record2', 'record3', 'record4'],
      aliasesMap: { $record: 'record' },
      queryParts: {
        record:
          '(any(value IN record.created WHERE value = true)) AND (any(value IN record.rating WHERE value = 5))',
        record1:
          'OPTIONAL MATCH (record)-[:COMMENT_TO_POST]->(record1:COMMENT) WHERE (any(value IN record1.authoredBy WHERE value =~ "(?i).*Sam.*") AND any(value IN record1.authoredBy WHERE value ENDS WITH "Altman"))',
        record2:
          'OPTIONAL MATCH (record1)--(record2:POST) WHERE (any(value IN record2.title WHERE value = "Hey"))',
        record3:
          'OPTIONAL MATCH (record)--(record3:POST) WHERE (any(value IN record3.title WHERE value = "Hey"))',
        record4:
          'OPTIONAL MATCH (record3)-[:COMMENT_TO_POST]->(record4:COMMENT) WHERE (any(value IN record4.authoredBy WHERE value =~ "(?i).*Sam.*") AND any(value IN record4.authoredBy WHERE value ENDS WITH "Altman"))'
      },
      where:
        'record IS NOT NULL AND (NOT((record1 IS NOT NULL AND record2 IS NOT NULL) OR (any(value IN record.title WHERE value <> "Forest")) OR (record3 IS NOT NULL AND record4 IS NOT NULL)))'
    })
  })

  it('parses subQueries correctly 5', () => {
    const result2 = parse(
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
      nodeAliases: ['record', 'record1', 'record2'],
      aliasesMap: { $record: 'record' },
      queryParts: {
        record: '',
        record1:
          'OPTIONAL MATCH (record)--(record1:COMMENT) WHERE (any(value IN record1.authoredBy WHERE value =~ "(?i).*Sam.*") AND any(value IN record1.authoredBy WHERE value ENDS WITH "Altman"))',
        record2:
          'OPTIONAL MATCH (record1)--(record2:POST) WHERE (any(value IN record2.title WHERE value = "Hey"))'
      },
      where: 'record IS NOT NULL AND (record1 IS NOT NULL AND record2 IS NOT NULL)'
    })
  })

  it('parses subQueries correctly 6', () => {
    const result6 = parse(
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
      nodeAliases: ['record', 'record1', 'record2'],
      aliasesMap: { $record: 'record' },
      queryParts: {
        record:
          '(any(value IN record.created WHERE value = true)) AND (any(value IN record.rating WHERE value = 5))',
        record1:
          'OPTIONAL MATCH (record)--(record1:CAR) WHERE (any(value IN record1.color WHERE value = "red"))',
        record2:
          'OPTIONAL MATCH (record)--(record2:SPOUSE) WHERE (any(value IN record2.gender WHERE value = "male"))'
      },
      where:
        'record IS NOT NULL AND (record1 IS NOT NULL OR record2 IS NOT NULL OR (any(value IN record.title WHERE value <> "Forest")))'
    })
  })

  it('parses subQueries correctly 7', () => {
    const result7 = parse(
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
      nodeAliases: ['record', 'record1', 'record2'],
      aliasesMap: { $record: 'record' },
      queryParts: {
        record:
          '(any(value IN record.created WHERE value = true)) AND (any(value IN record.rating WHERE value = 5))',
        record1:
          'OPTIONAL MATCH (record)--(record1:CAR) WHERE (any(value IN record1.color WHERE value = "red"))',
        record2:
          'OPTIONAL MATCH (record1)--(record2:SPOUSE) WHERE (any(value IN record2.gender WHERE value = "male"))'
      },
      where:
        'record IS NOT NULL AND ((record1 IS NOT NULL AND (record2 IS NOT NULL XOR (any(value IN record1.title WHERE value <> "Forest")))))'
    })
  })

  it('parses subQueries correctly 8', () => {
    const result8 = parse(
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
      nodeAliases: ['record', 'record1', 'record2'],
      aliasesMap: { $record: 'record' },
      queryParts: {
        record:
          '(any(value IN record.created WHERE value = true)) AND (any(value IN record.rating WHERE value = 5))',
        record1:
          'OPTIONAL MATCH (record)--(record1:CAR) WHERE (any(value IN record1.color WHERE value = "red"))',
        record2:
          'OPTIONAL MATCH (record1)--(record2:SPOUSE) WHERE (any(value IN record2.gender WHERE value = "male"))'
      },
      where:
        'record IS NOT NULL AND ((record1 IS NOT NULL AND (record2 IS NOT NULL XOR (any(value IN record1.title WHERE value <> "Forest")))))'
    })
  })

  it('parses subQueries correctly 9', () => {
    const result9 = parse(
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
      nodeAliases: ['record', 'record1', 'record2'],
      aliasesMap: { $record: 'record' },
      queryParts: {
        record: '',
        record1:
          'OPTIONAL MATCH (record)--(record1:CAR) WHERE (any(value IN record1.color WHERE value = "red"))',
        record2:
          'OPTIONAL MATCH (record)--(record2:SPOUSE) WHERE (any(value IN record2.gender WHERE value = "male"))'
      },
      where: 'record IS NOT NULL AND (record1 IS NOT NULL OR record2 IS NOT NULL)'
    })
  })

  it('parses ID criteria correctly 10', () => {
    const result10 = parse(
      {
        [ID_CLAUSE_OPERATOR]: '123'
      },
      queryBuilderOptions
    )
    expect(result10).toEqual({
      nodeAliases: ['record'],
      aliasesMap: { $record: 'record' },
      queryParts: {
        record: `(any(value IN record.${RUSHDB_KEY_ID} WHERE value = "123"))`
      },
      where: 'record IS NOT NULL'
    })
  })

  it('parses nested ID criteria correctly 11', () => {
    const result10 = parse(
      {
        [ID_CLAUSE_OPERATOR]: '123',
        CAR: {
          [ID_CLAUSE_OPERATOR]: '567'
        }
      },
      queryBuilderOptions
    )
    expect(result10).toEqual({
      nodeAliases: ['record', 'record1'],
      aliasesMap: { $record: 'record' },
      queryParts: {
        record: '(any(value IN record.__RUSHDB__KEY__ID__ WHERE value = "123"))',
        record1:
          'OPTIONAL MATCH (record)--(record1:CAR) WHERE (any(value IN record1.__RUSHDB__KEY__ID__ WHERE value = "567"))'
      },
      where: 'record IS NOT NULL AND record1 IS NOT NULL'
    })
  })

  it('parses empty where correctly 12', () => {
    const result10 = parse({}, queryBuilderOptions)
    expect(result10).toEqual({
      nodeAliases: ['record'],
      aliasesMap: { $record: 'record' },
      queryParts: {
        record: ''
      },
      where: 'record IS NOT NULL'
    })
  })

  it('parses empty nested criteria correctly 13', () => {
    const result10 = parse(
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
      nodeAliases: ['record', 'record1', 'record2', 'record3'],
      aliasesMap: { $record: 'record' },
      queryParts: {
        record: '',
        record1:
          'OPTIONAL MATCH (record)--(record1:CAR) WHERE (any(value IN record1.color WHERE value = "red"))',
        record2:
          'OPTIONAL MATCH (record)--(record2:SPOUSE) WHERE (any(value IN record2.gender WHERE value = "male"))',
        record3: 'OPTIONAL MATCH (record2)--(record3:CAR)'
      },
      where:
        'record IS NOT NULL AND (record1 IS NOT NULL OR (record2 IS NOT NULL AND (NOT(record3 IS NOT NULL))))'
    })
  })

  it('parses logical grouping on top level correctly 14', () => {
    const result10 = parse(
      {
        $xor: [{ foundingDate: { $gte: '1989-10-01T19:05:17.780Z' } }, { city: 'Doral' }]
      },
      queryBuilderOptions
    )
    expect(result10).toEqual({
      nodeAliases: ['record'],
      aliasesMap: { $record: 'record' },
      queryParts: {
        record:
          '((any(value IN record.foundingDate WHERE apoc.convert.fromJsonMap(`record`.`__RUSHDB__KEY__PROPERTIES__META__`).`foundingDate` = "datetime" AND datetime(value) >= datetime("1989-10-01T19:05:17.780Z"))) XOR (any(value IN record.city WHERE value = "Doral")))'
      },
      where: 'record IS NOT NULL'
    })
  })
})
