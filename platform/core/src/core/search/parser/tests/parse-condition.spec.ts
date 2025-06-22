// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-nocheck
import { RUSHDB_KEY_PROPERTIES_META, RUSHDB_VALUE_NULL } from '@/core/common/constants'
import { QueryCriteriaParsingError } from '@/core/search/parser/errors'
import { parseComparison } from '@/core/search/parser/parseComparison'
import { TSearchQueryBuilderOptions } from '@/core/search/search.types'

describe('parseComparison', () => {
  const queryBuilderOptions: TSearchQueryBuilderOptions = {
    nodeAlias: 'record0'
  }

  it('parses complex conditions correctly', () => {
    const result0 = parseComparison('name', 'Jack', queryBuilderOptions)
    expect(result0).toEqual(`any(value IN ${queryBuilderOptions.nodeAlias}.name WHERE value = "Jack")`)

    const result1 = parseComparison('age', { $gte: 21, $lte: 100 }, queryBuilderOptions)
    expect(result1).toEqual([
      `any(value IN ${queryBuilderOptions.nodeAlias}.age WHERE value >= 21)`,
      `any(value IN ${queryBuilderOptions.nodeAlias}.age WHERE value <= 100)`
    ])

    const result2 = parseComparison(
      'address',
      {
        $startsWith: 'Lo',
        $contains: 'n',
        $endsWith: 'don'
      },
      queryBuilderOptions
    )
    expect(result2).toEqual([
      `any(value IN ${queryBuilderOptions.nodeAlias}.address WHERE value STARTS WITH \"Lo\")`,
      `any(value IN ${queryBuilderOptions.nodeAlias}.address WHERE value =~ "(?i).*n.*")`,
      `any(value IN ${queryBuilderOptions.nodeAlias}.address WHERE value ENDS WITH \"don\")`
    ])

    const result3 = parseComparison(
      'confirmed',
      {
        $ne: true
      },
      queryBuilderOptions
    )
    expect(result3).toEqual([`any(value IN ${queryBuilderOptions.nodeAlias}.confirmed WHERE value <> true)`])

    const result4 = parseComparison(
      'dob',
      {
        $year: 1922,
        $month: 1,
        $day: 31
      },
      queryBuilderOptions
    )
    expect(result4).toEqual(
      `any(value IN ${queryBuilderOptions.nodeAlias}.dob WHERE apoc.convert.fromJsonMap(\`${queryBuilderOptions.nodeAlias}\`.\`${RUSHDB_KEY_PROPERTIES_META}\`).\`dob\` = "datetime" AND datetime(value) = datetime({year: 1922, month: 1, day: 31}))`
    )
  })

  it('parses $in and $nin operators correctly', () => {
    const result0 = parseComparison('status', { $in: ['active', 'inactive'] }, queryBuilderOptions)
    expect(result0).toEqual([
      `any(value IN ${queryBuilderOptions.nodeAlias}.status WHERE value IN ["active", "inactive"])`
    ])

    const result1 = parseComparison('status', { $nin: ['active', 'inactive'] }, queryBuilderOptions)
    expect(result1).toEqual([
      `none(value IN ${queryBuilderOptions.nodeAlias}.status WHERE value IN ["active", "inactive"])`
    ])
  })

  it('parses mixed datetime operators correctly', () => {
    const result0 = parseComparison(
      'createdAt',
      {
        $gte: { $year: 2020, $month: 1, $day: 1 },
        $lt: { $year: 2021, $month: 1, $day: 1 }
      },
      queryBuilderOptions
    )
    expect(result0).toEqual([
      `any(value IN ${queryBuilderOptions.nodeAlias}.createdAt WHERE apoc.convert.fromJsonMap(\`${queryBuilderOptions.nodeAlias}\`.\`${RUSHDB_KEY_PROPERTIES_META}\`).\`createdAt\` = "datetime" AND datetime(value) >= datetime({year: 2020, month: 1, day: 1}))`,
      `any(value IN ${queryBuilderOptions.nodeAlias}.createdAt WHERE apoc.convert.fromJsonMap(\`${queryBuilderOptions.nodeAlias}\`.\`${RUSHDB_KEY_PROPERTIES_META}\`).\`createdAt\` = "datetime" AND datetime(value) < datetime({year: 2021, month: 1, day: 1}))`
    ])
  })

  it('throws error for unsupported operators', () => {
    expect(() => {
      // eslint-disable-next-line @typescript-eslint/ban-ts-comment
      // @ts-ignore
      parseComparison('name', { $unknown: 'value' }, queryBuilderOptions)
    }).toThrow(QueryCriteriaParsingError)
  })

  it('throws error for invalid datetime objects', () => {
    expect(() => {
      // eslint-disable-next-line @typescript-eslint/ban-ts-comment
      // @ts-ignore
      parseComparison('createdAt', { $year: 2020, $unknown: 5 }, queryBuilderOptions)
    }).toThrow(QueryCriteriaParsingError)
  })

  it('throws error for array in data', () => {
    expect(() => {
      // eslint-disable-next-line @typescript-eslint/ban-ts-comment
      // @ts-ignore
      parseComparison('title', ['Hi', 'Bye'], queryBuilderOptions)
    }).toThrow(QueryCriteriaParsingError)
  })

  it('handles null values correctly', () => {
    const result = parseComparison('field', null, queryBuilderOptions)
    expect(result).toEqual(
      `any(value IN ${queryBuilderOptions.nodeAlias}.field WHERE value = \"${RUSHDB_VALUE_NULL}\")`
    )
  })

  it('parses $exists operator correctly', () => {
    const result0 = parseComparison('field', { $exists: true }, queryBuilderOptions)
    expect(result0).toEqual([
      `${queryBuilderOptions.nodeAlias}.field IS NOT NULL AND size(${queryBuilderOptions.nodeAlias}.field) > 0`
    ])

    const result1 = parseComparison('field', { $exists: false }, queryBuilderOptions)
    expect(result1).toEqual([
      `(${queryBuilderOptions.nodeAlias}.field IS NULL OR size(${queryBuilderOptions.nodeAlias}.field) = 0)`
    ])
  })

  it('throws error for invalid $exists value', () => {
    expect(() => {
      // eslint-disable-next-line @typescript-eslint/ban-ts-comment
      // @ts-ignore
      parseComparison('field', { $exists: 'invalid' }, queryBuilderOptions)
    }).toThrow(QueryCriteriaParsingError)
  })

  it('parses $type operator correctly', () => {
    const result0 = parseComparison('field', { $type: 'string' }, queryBuilderOptions)
    expect(result0).toEqual([
      `apoc.convert.fromJsonMap(\`${queryBuilderOptions.nodeAlias}\`.\`${RUSHDB_KEY_PROPERTIES_META}\`).\`field\` = "string"`
    ])

    const result1 = parseComparison('age', { $type: 'number' }, queryBuilderOptions)
    expect(result1).toEqual([
      `apoc.convert.fromJsonMap(\`${queryBuilderOptions.nodeAlias}\`.\`${RUSHDB_KEY_PROPERTIES_META}\`).\`age\` = "number"`
    ])

    const result2 = parseComparison('embedding', { $type: 'vector' }, queryBuilderOptions)
    expect(result2).toEqual([
      `apoc.convert.fromJsonMap(\`${queryBuilderOptions.nodeAlias}\`.\`${RUSHDB_KEY_PROPERTIES_META}\`).\`embedding\` = "vector"`
    ])
  })

  it('throws error for invalid $type value', () => {
    expect(() => {
      // eslint-disable-next-line @typescript-eslint/ban-ts-comment
      // @ts-ignore
      parseComparison('field', { $type: 123 }, queryBuilderOptions)
    }).toThrow(QueryCriteriaParsingError)
  })
})
