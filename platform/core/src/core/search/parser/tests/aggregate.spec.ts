import { DEFAULT_RECORD_ALIAS } from '@/core/common/constants'
import { Aggregate } from '@/core/common/types'
import { buildAggregation, parseBottomUpQuery } from '@/core/search/parser/aggregate'

describe('Aggregate', () => {
  it('aggregates correctly 1', () => {
    const queryAggregate: Aggregate = {
      projectName: '$record.name',
      employee_count: { fn: 'count', alias: '$employee' },
      departments: {
        fn: 'collect',
        uniq: true,
        field: 'name',
        alias: '$department'
      },
      totalStoryPoints: { fn: 'sum', field: 'storyPoints', alias: '$task' }
    }

    const result1 = buildAggregation(queryAggregate, {
      $employee: 'record2',
      $department: 'record1',
      $task: 'record3'
    })

    expect(result1).toEqual({
      recordPart:
        'collect(DISTINCT record {__RUSHDB__KEY__ID__: record.__RUSHDB__KEY__ID__, __RUSHDB__KEY__PROPERTIES__META__: record.__RUSHDB__KEY__PROPERTIES__META__, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], `projectName`: record.`name`, `employee_count`, `departments`, `totalStoryPoints`}) AS records',
      withPart:
        'WITH record, count(record2) AS `employee_count`, apoc.coll.sortMaps(collect(DISTINCT record1.`name`), "__RUSHDB__KEY__ID__")[0..100] AS `departments`, sum(record3.`storyPoints`) AS `totalStoryPoints`'
    })
  })

  it('aggregates correctly 2', () => {
    const queryAggregate: Aggregate = {
      departments: {
        fn: 'collect',
        uniq: true,
        alias: '$department',
        aggregate: {
          projects: {
            fn: 'collect',
            uniq: true,
            alias: '$project',
            aggregate: {
              employees: {
                fn: 'collect',
                uniq: true,
                alias: '$employee'
              }
            }
          }
        }
      }
    }

    const result1 = parseBottomUpQuery(queryAggregate, DEFAULT_RECORD_ALIAS, {
      $employee: 'record3',
      $department: 'record1',
      $project: 'record2',
      $task: 'record4'
    })

    expect(result1).toEqual([
      {
        recordVariables: ['record', 'record1', 'record2'],
        withStatement:
          'WITH record, record1, record2, apoc.coll.sortMaps(collect(DISTINCT record3 {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record3) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "__RUSHDB__KEY__ID__")[0..100] AS `employees`'
      },
      {
        recordVariables: ['record', 'record1'],
        withStatement:
          'WITH record, record1, apoc.coll.sortMaps(collect(DISTINCT record2 {.*, `employees`, __RUSHDB__KEY__LABEL__: [label IN labels(record2) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "__RUSHDB__KEY__ID__")[0..100] AS `projects`'
      },
      {
        recordVariables: ['record'],
        withStatement:
          'WITH record, apoc.coll.sortMaps(collect(DISTINCT record1 {.*, `projects`, __RUSHDB__KEY__LABEL__: [label IN labels(record1) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "__RUSHDB__KEY__ID__")[0..100] AS `departments`'
      }
    ])
  })

  it('aggregates correctly 3', () => {
    const queryAggregate: Aggregate = {
      departments: {
        fn: 'collect',
        uniq: true,
        alias: '$department',
        aggregate: {
          projects: {
            fn: 'collect',
            uniq: true,
            alias: '$project',
            aggregate: {
              employees: {
                fn: 'collect',
                uniq: true,
                alias: '$employee',
                orderBy: { salary: 'asc' },
                limit: 5
              },
              tasks: {
                fn: 'collect',
                uniq: true,
                alias: '$task'
              }
            }
          }
        }
      }
    }

    const result1 = parseBottomUpQuery(queryAggregate, DEFAULT_RECORD_ALIAS, {
      $employee: 'record3',
      $department: 'record1',
      $project: 'record2',
      $task: 'record4'
    })

    expect(result1).toEqual([
      {
        recordVariables: ['record', 'record1', 'record2'],
        withStatement:
          'WITH record, record1, record2, apoc.coll.sortMaps(collect(DISTINCT record3 {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record3) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "^salary")[0..5] AS `employees`, apoc.coll.sortMaps(collect(DISTINCT record4 {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record4) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "__RUSHDB__KEY__ID__")[0..100] AS `tasks`'
      },
      {
        recordVariables: ['record', 'record1'],
        withStatement:
          'WITH record, record1, apoc.coll.sortMaps(collect(DISTINCT record2 {.*, `employees`, `tasks`, __RUSHDB__KEY__LABEL__: [label IN labels(record2) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "__RUSHDB__KEY__ID__")[0..100] AS `projects`'
      },
      {
        recordVariables: ['record'],
        withStatement:
          'WITH record, apoc.coll.sortMaps(collect(DISTINCT record1 {.*, `projects`, __RUSHDB__KEY__LABEL__: [label IN labels(record1) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "__RUSHDB__KEY__ID__")[0..100] AS `departments`'
      }
    ])
  })
})
