// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-nocheck
import { QueryBuilder } from '@/common/QueryBuilder'
import { RUSHDB_LABEL_RECORD } from '@/core/common/constants'
import { SearchDto } from '@/core/search/dto/search.dto'
import { buildAggregation, buildQuery } from '@/core/search/parser'
import { buildRelatedQueryPart } from '@/core/search/parser/buildRelatedRecordQueryPart'
import { projectIdInline } from '@/core/search/parser/projectIdInline'

const buildQ = ({ id, searchParams }: { searchParams?: SearchDto; id?: string }) => {
  const relatedQueryPart = buildRelatedQueryPart(id)

  const { queryClauses, parsedWhere, aliasesMap } = buildQuery(searchParams)

  const { withPart: aggregateProjections, recordPart: returnPart } = buildAggregation(
    searchParams.aggregate,
    aliasesMap
  )

  const labelPart =
    searchParams.labels && searchParams.labels.length === 1 ? `:${searchParams.labels?.[0]}` : ''

  const queryBuilder = new QueryBuilder()

  queryBuilder
    .append(`MATCH ${relatedQueryPart}(record:${RUSHDB_LABEL_RECORD}${labelPart} { ${projectIdInline()} })`)
    .append(queryClauses)
    .append(`WITH ${parsedWhere.nodeAliases.join(', ')}`)
    .append(`WHERE ${parsedWhere.where}`)
    .append(aggregateProjections)
    .append(`RETURN ${returnPart}`)

  return queryBuilder.getQuery()
}

const q0 = {
  labels: ['COMPANY'],
  where: {
    // stage: { $or: ['seed', 'roundA'] },
    $or: [{ stage: 'seed' }, { stage: 'roundA' }],
    EMPLOYEE: {
      $alias: '$employee',
      salary: {
        $gte: 500_000
      }
    }
  },
  aggregate: {
    employees: {
      fn: 'collect',
      orderBy: {
        salary: 'desc'
      },
      alias: '$employee',
      limit: 10
    }
  }
}

const r0 = `MATCH (record:__RUSHDB__LABEL__RECORD__:COMPANY { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE ((any(value IN record.stage WHERE value = "seed")) OR (any(value IN record.stage WHERE value = "roundA"))) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:EMPLOYEE) WHERE (any(value IN record1.salary WHERE value >= 500000))
WITH record, record1
WHERE record IS NOT NULL AND record1 IS NOT NULL
WITH record, apoc.coll.sortMaps(collect(DISTINCT record1 {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record1) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "salary")[0..10] AS \`employees\`
RETURN collect(DISTINCT record {__RUSHDB__KEY__ID__: record.__RUSHDB__KEY__ID__, __RUSHDB__KEY__PROPERTIES__META__: record.__RUSHDB__KEY__PROPERTIES__META__, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`employees\`}) AS records`

const q1 = {
  where: {
    rating: {
      $gte: 1
    },
    departments: {
      $alias: '$department',
      projects: {
        $alias: '$project',
        employees: {
          $alias: '$employee',
          salary: {
            $gte: 499500
          }
        }
      }
    }
  },
  orderBy: {},
  skip: 0,
  limit: 100,
  labels: ['COMPANY'],
  aggregate: {
    departments: {
      fn: 'collect',
      alias: '$department',
      aggregate: {
        projects: {
          fn: 'collect',
          alias: '$project',
          orderBy: {
            projectName: 'asc',
            projectId: 'desc'
          },
          aggregate: {
            employees: {
              fn: 'collect',
              orderBy: {
                salary: 'desc'
              },
              alias: '$employee',
              limit: 3
            }
          }
        }
      }
    }
  }
}

const r1 = `MATCH (record:__RUSHDB__LABEL__RECORD__:COMPANY { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE (any(value IN record.rating WHERE value >= 1)) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:departments)
OPTIONAL MATCH (record1)--(record2:projects)
OPTIONAL MATCH (record2)--(record3:employees) WHERE (any(value IN record3.salary WHERE value >= 499500))
WITH record, record1, record2, record3
WHERE record IS NOT NULL AND (record1 IS NOT NULL AND (record2 IS NOT NULL AND record3 IS NOT NULL))
WITH record, record1, record2, apoc.coll.sortMaps(collect(DISTINCT record3 {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record3) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "salary")[0..3] AS \`employees\`
WITH record, record1, apoc.coll.sortMaps(collect(DISTINCT record2 {.*, \`employees\`, __RUSHDB__KEY__LABEL__: [label IN labels(record2) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "^projectName")[0..100] AS \`projects\`
WITH record, apoc.coll.sortMaps(collect(DISTINCT record1 {.*, \`projects\`, __RUSHDB__KEY__LABEL__: [label IN labels(record1) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "__RUSHDB__KEY__ID__")[0..100] AS \`departments\`
RETURN collect(DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`departments\`}) AS records`

const q2 = {
  where: {
    rating: {
      $gte: 1
    },
    departments: {
      $alias: '$department',
      projects: {
        $alias: '$project',
        employees: {
          $alias: '$employee',
          salary: {
            $gte: 499500
          }
        }
      }
    }
  },
  orderBy: {},
  skip: 0,
  limit: 100,
  labels: ['COMPANY'],
  aggregate: {
    companyName: '$record.name',
    employeesCount: { fn: 'count', uniq: true, alias: '$employee' },
    totalWage: { fn: 'sum', field: 'salary', alias: '$employee' },
    avgSalary: {
      fn: 'avg',
      field: 'salary',
      alias: '$employee',
      precision: 0
    },
    minSalary: { fn: 'min', field: 'salary', alias: '$employee' },
    maxSalary: { fn: 'max', field: 'salary', alias: '$employee' }
  }
}

const r2 = `MATCH (record:__RUSHDB__LABEL__RECORD__:COMPANY { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE (any(value IN record.rating WHERE value >= 1)) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:departments)
OPTIONAL MATCH (record1)--(record2:projects)
OPTIONAL MATCH (record2)--(record3:employees) WHERE (any(value IN record3.salary WHERE value >= 499500))
WITH record, record1, record2, record3
WHERE record IS NOT NULL AND (record1 IS NOT NULL AND (record2 IS NOT NULL AND record3 IS NOT NULL))
WITH record, count(DISTINCT record3) AS \`employeesCount\`, sum(record3.\`salary\`) AS \`totalWage\`, toInteger(avg(record3.\`salary\`)) AS \`avgSalary\`, min(record3.\`salary\`) AS \`minSalary\`, max(record3.\`salary\`) AS \`maxSalary\`
RETURN collect(DISTINCT record {__RUSHDB__KEY__ID__: record.__RUSHDB__KEY__ID__, __RUSHDB__KEY__PROPERTIES__META__: record.__RUSHDB__KEY__PROPERTIES__META__, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`companyName\`: record.\`name\`, \`employeesCount\`, \`totalWage\`, \`avgSalary\`, \`minSalary\`, \`maxSalary\`}) AS records`

const q3 = {
  where: {
    tag: 'top-sellers',
    AUTHOR: {
      name: {
        $or: [
          {
            $startsWith: 'Jack',
            $endsWith: 'Rooney'
          },
          {
            $year: 1984
          }
        ]
      },
      $relation: {
        type: 'AUTHORED',
        direction: 'in'
      }
    },
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
      },
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
        },
        {
          JOB: {
            title: 'Manager'
          }
        }
      ]
    },
    departments: {
      $alias: '$department',
      projects: {
        $alias: '$project',
        employees: {
          $alias: '$employee',
          salary: {
            $gte: 499500
          }
        }
      }
    }
  },
  orderBy: {},
  skip: 0,
  limit: 100,
  labels: ['COMPANY'],
  aggregate: {
    companyName: '$record.name',
    employeesCount: { fn: 'count', uniq: true, alias: '$employee' },
    totalWage: { fn: 'sum', field: 'salary', alias: '$employee' },
    avgSalary: {
      fn: 'avg',
      field: 'salary',
      alias: '$employee',
      precision: 0
    },
    minSalary: { fn: 'min', field: 'salary', alias: '$employee' },
    maxSalary: { fn: 'max', field: 'salary', alias: '$employee' }
  }
}

const r3 = `MATCH (record:__RUSHDB__LABEL__RECORD__:COMPANY { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE (any(value IN record.tag WHERE value = "top-sellers")) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)<-[:AUTHORED]-(record1:AUTHOR) WHERE ((any(value IN record1.name WHERE value STARTS WITH "Jack") AND any(value IN record1.name WHERE value ENDS WITH "Rooney") OR any(value IN record1.name WHERE apoc.convert.fromJsonMap(\`record1\`.\`__RUSHDB__KEY__PROPERTIES__META__\`).\`name\` = "datetime" AND datetime(value) = datetime({year: 1984}))))
OPTIONAL MATCH (record)--(record2:POST) WHERE (any(value IN record2.created WHERE apoc.convert.fromJsonMap(\`record2\`.\`__RUSHDB__KEY__PROPERTIES__META__\`).\`created\` = "datetime" AND datetime(value) = datetime({year: 2011, month: 11, day: 11}))) AND (((any(value IN record2.rating WHERE value > 4.5) AND any(value IN record2.rating WHERE value < 6)) OR any(value IN record2.rating WHERE value <> 3) OR (NOT(any(value IN record2.rating WHERE value >= 4))))) AND (any(value IN record2.title WHERE value <> "Forest"))
OPTIONAL MATCH (record2)-[:COMMENT_TO_POST]->(record3:COMMENT) WHERE (any(value IN record3.authoredBy WHERE value =~ "(?i).*Sam.*"))
OPTIONAL MATCH (record2)--(record4:CAR) WHERE (any(value IN record4.color WHERE value = "red"))
OPTIONAL MATCH (record4)--(record5:SPOUSE) WHERE (any(value IN record5.gender WHERE value = "male"))
OPTIONAL MATCH (record2)--(record6:JOB) WHERE (any(value IN record6.title WHERE value = "Manager"))
OPTIONAL MATCH (record)--(record7:departments)
OPTIONAL MATCH (record7)--(record8:projects)
OPTIONAL MATCH (record8)--(record9:employees) WHERE (any(value IN record9.salary WHERE value >= 499500))
WITH record, record1, record2, record3, record4, record5, record6, record7, record8, record9
WHERE record IS NOT NULL AND record1 IS NOT NULL AND (record2 IS NOT NULL AND record3 IS NOT NULL) AND (record4 IS NOT NULL AND (record5 IS NOT NULL AND record6 IS NOT NULL))
WITH record, count(DISTINCT record9) AS \`employeesCount\`, sum(record9.\`salary\`) AS \`totalWage\`, toInteger(avg(record9.\`salary\`)) AS \`avgSalary\`, min(record9.\`salary\`) AS \`minSalary\`, max(record9.\`salary\`) AS \`maxSalary\`
RETURN collect(DISTINCT record {__RUSHDB__KEY__ID__: record.__RUSHDB__KEY__ID__, __RUSHDB__KEY__PROPERTIES__META__: record.__RUSHDB__KEY__PROPERTIES__META__, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`companyName\`: record.\`name\`, \`employeesCount\`, \`totalWage\`, \`avgSalary\`, \`minSalary\`, \`maxSalary\`}) AS records`

const q4 = {
  labels: ['COMPANY'],
  where: {
    stage: { $or: ['seed', 'roundA'] }
  }
}

const r4 = `MATCH (record:__RUSHDB__LABEL__RECORD__:COMPANY { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE ((any(value IN record.stage WHERE value = "seed") OR any(value IN record.stage WHERE value = "roundA"))) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
WITH record
WHERE record IS NOT NULL
RETURN collect(DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}) AS records`

const q5 = {
  labels: ['COMPANY'],
  where: {
    $or: [{ stage: 'seed' }, { stage: 'roundA' }]
  }
}

const r5 = `MATCH (record:__RUSHDB__LABEL__RECORD__:COMPANY { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE ((any(value IN record.stage WHERE value = "seed")) OR (any(value IN record.stage WHERE value = "roundA"))) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
WITH record
WHERE record IS NOT NULL
RETURN collect(DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}) AS records`

const q6 = {
  labels: ['COMPANY'],
  where: {
    $id: { $or: ['1234567890', '0987654321'] },
    name: 'alex',
    USER: {
      $id: { $in: ['1234567890', '0987654321'] }
    }
  }
}

const r6 = `MATCH (record:__RUSHDB__LABEL__RECORD__:COMPANY { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE ((any(value IN record.__RUSHDB__KEY__ID__ WHERE value = "1234567890") OR any(value IN record.__RUSHDB__KEY__ID__ WHERE value = "0987654321"))) AND (any(value IN record.name WHERE value = "alex")) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:USER) WHERE (any(value IN record1.__RUSHDB__KEY__ID__ WHERE value IN ["1234567890", "0987654321"]))
WITH record, record1
WHERE record IS NOT NULL AND record1 IS NOT NULL
RETURN collect(DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}) AS records`

const q7 = {
  where: {
    rating: {
      $gte: 1
    },
    departments: {
      $alias: '$department'
    }
  },
  orderBy: {},
  skip: 0,
  limit: 100,
  labels: ['COMPANY'],
  aggregate: {
    tags: {
      fn: 'collect',
      alias: '$department',
      field: 'tags'
    }
  }
}

const r7 = `MATCH (record:__RUSHDB__LABEL__RECORD__:COMPANY { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE (any(value IN record.rating WHERE value >= 1)) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:departments)
WITH record, record1
WHERE record IS NOT NULL AND record1 IS NOT NULL
WITH record, apoc.coll.toSet(apoc.coll.removeAll(apoc.coll.sort(apoc.coll.flatten(collect(DISTINCT record1.\`tags\`))), ["__RUSHDB__VALUE__EMPTY__ARRAY__"]))[0..100] AS \`tags\`
RETURN collect(DISTINCT record {__RUSHDB__KEY__ID__: record.__RUSHDB__KEY__ID__, __RUSHDB__KEY__PROPERTIES__META__: record.__RUSHDB__KEY__PROPERTIES__META__, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`tags\`}) AS records`

const q8 = {
  where: {
    departments: {
      $alias: '$department'
    }
  },
  orderBy: {},
  skip: 0,
  limit: 100,
  labels: ['COMPANY'],
  aggregate: {
    departmentId: `$department.__id`
  }
}

const r8 = `MATCH (record:__RUSHDB__LABEL__RECORD__:COMPANY { __RUSHDB__KEY__PROJECT__ID__: $projectId })
ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:departments)
WITH record, record1
WHERE record IS NOT NULL AND record1 IS NOT NULL
RETURN collect(DISTINCT record {__RUSHDB__KEY__ID__: record.__RUSHDB__KEY__ID__, __RUSHDB__KEY__PROPERTIES__META__: record.__RUSHDB__KEY__PROPERTIES__META__, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`departmentId\`: record1.\`__RUSHDB__KEY__ID__\`}) AS records`

describe('build complete query', () => {
  it('0', () => {
    const result = buildQ({ searchParams: q0 })

    expect(result).toEqual(r0)
  })

  it('1', () => {
    const result = buildQ({ searchParams: q1 })

    expect(result).toEqual(r1)
  })

  it('2', () => {
    const result = buildQ({ searchParams: q2 })

    expect(result).toEqual(r2)
  })

  it('3', () => {
    const result = buildQ({ searchParams: q3 })

    expect(result).toEqual(r3)
  })

  it('4', () => {
    const result = buildQ({ searchParams: q4 })

    expect(result).toEqual(r4)
  })

  it('5', () => {
    const result = buildQ({ searchParams: q5 })

    expect(result).toEqual(r5)
  })

  it('6', () => {
    const result = buildQ({ searchParams: q6 })

    expect(result).toEqual(r6)
  })

  it('7', () => {
    const result = buildQ({ searchParams: q7 })

    expect(result).toEqual(r7)
  })

  it('8', () => {
    const result = buildQ({ searchParams: q8 })

    expect(result).toEqual(r8)
  })
})
