// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-nocheck

import { EntityQueryService } from '@/core/entity/entity-query.service'

const q0 = {
  labels: ['COMPANY'],
  where: {
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

const r0 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE ((any(value IN record.stage WHERE value = "seed")) OR (any(value IN record.stage WHERE value = "roundA"))) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:EMPLOYEE) WHERE (any(value IN record1.salary WHERE value >= 500000))
WITH record, record1 WHERE record IS NOT NULL AND record1 IS NOT NULL
WITH record, apoc.coll.sortMaps(collect(DISTINCT record1 {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record1) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "salary")[0..10] AS \`employees\`
RETURN collect(DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`employees\`}) AS records`

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

const r1 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE (any(value IN record.rating WHERE value >= 1)) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:departments)
OPTIONAL MATCH (record1)--(record2:projects)
OPTIONAL MATCH (record2)--(record3:employees) WHERE (any(value IN record3.salary WHERE value >= 499500))
WITH record, record1, record2, record3 WHERE record IS NOT NULL AND (record1 IS NOT NULL AND (record2 IS NOT NULL AND record3 IS NOT NULL))
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

const r2 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE (any(value IN record.rating WHERE value >= 1)) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:departments)
OPTIONAL MATCH (record1)--(record2:projects)
OPTIONAL MATCH (record2)--(record3:employees) WHERE (any(value IN record3.salary WHERE value >= 499500))
WITH record, record1, record2, record3 WHERE record IS NOT NULL AND (record1 IS NOT NULL AND (record2 IS NOT NULL AND record3 IS NOT NULL))
WITH record, count(DISTINCT record3) AS \`employeesCount\`, sum(record3.\`salary\`) AS \`totalWage\`, toInteger(avg(record3.\`salary\`)) AS \`avgSalary\`, min(record3.\`salary\`) AS \`minSalary\`, max(record3.\`salary\`) AS \`maxSalary\`
RETURN collect(DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`companyName\`: record.\`name\`, \`employeesCount\`, \`totalWage\`, \`avgSalary\`, \`minSalary\`, \`maxSalary\`}) AS records`

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

const r3 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
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
WITH record, record1, record2, record3, record4, record5, record6, record7, record8, record9 WHERE record IS NOT NULL AND record1 IS NOT NULL AND (record2 IS NOT NULL AND record3 IS NOT NULL) AND (record4 IS NOT NULL AND (record5 IS NOT NULL AND record6 IS NOT NULL))
WITH record, count(DISTINCT record9) AS \`employeesCount\`, sum(record9.\`salary\`) AS \`totalWage\`, toInteger(avg(record9.\`salary\`)) AS \`avgSalary\`, min(record9.\`salary\`) AS \`minSalary\`, max(record9.\`salary\`) AS \`maxSalary\`
RETURN collect(DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`companyName\`: record.\`name\`, \`employeesCount\`, \`totalWage\`, \`avgSalary\`, \`minSalary\`, \`maxSalary\`}) AS records`

const q4 = {
  labels: ['COMPANY'],
  where: {
    stage: { $or: ['seed', 'roundA'] }
  }
}

const r4 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE ((any(value IN record.stage WHERE value = "seed") OR any(value IN record.stage WHERE value = "roundA"))) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
RETURN collect(DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}) AS records`

const q5 = {
  labels: ['COMPANY'],
  where: {
    $or: [{ stage: 'seed' }, { stage: 'roundA' }]
  }
}

const r5 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE ((any(value IN record.stage WHERE value = "seed")) OR (any(value IN record.stage WHERE value = "roundA"))) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
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

const r6 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE ((any(value IN record.__RUSHDB__KEY__ID__ WHERE value = "1234567890") OR any(value IN record.__RUSHDB__KEY__ID__ WHERE value = "0987654321"))) AND (any(value IN record.name WHERE value = "alex")) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:USER) WHERE (any(value IN record1.__RUSHDB__KEY__ID__ WHERE value IN ["1234567890", "0987654321"]))
WITH record, record1 WHERE record IS NOT NULL AND record1 IS NOT NULL
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

const r7 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE (any(value IN record.rating WHERE value >= 1)) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:departments)
WITH record, record1 WHERE record IS NOT NULL AND record1 IS NOT NULL
WITH record, apoc.coll.toSet(apoc.coll.removeAll(apoc.coll.sort(apoc.coll.flatten(collect(DISTINCT record1.\`tags\`))), ["__RUSHDB__VALUE__EMPTY__ARRAY__"]))[0..100] AS \`tags\`
RETURN collect(DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`tags\`}) AS records`

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

const r8 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:departments)
WITH record, record1 WHERE record IS NOT NULL AND record1 IS NOT NULL
RETURN collect(DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`departmentId\`: record1.\`__RUSHDB__KEY__ID__\`}) AS records`

const q9 = {
  where: {
    emb: {
      $vector: {
        fn: 'gds.similarity.cosine',
        query: [1, 2, 3, 4, 5],
        threshold: {
          $gte: 0.5,
          $lte: 0.8,
          $ne: 0.75
        }
      }
    }
  }
}

const r9 = `MATCH (record:__RUSHDB__LABEL__RECORD__ { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE ((\`record\`.\`emb\` IS NOT NULL AND apoc.convert.fromJsonMap(\`record\`.\`__RUSHDB__KEY__PROPERTIES__META__\`).\`emb\` = "vector" AND gds.similarity.cosine(\`record\`.\`emb\`, [1,2,3,4,5]) >= 0.5 AND gds.similarity.cosine(\`record\`.\`emb\`, [1,2,3,4,5]) <= 0.8 AND gds.similarity.cosine(\`record\`.\`emb\`, [1,2,3,4,5]) <> 0.75)) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
RETURN collect(DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}) AS records`

const q10 = {
  where: {
    DOCUMENT: {
      CHUNK: {
        embedding: {
          $vector: {
            fn: 'gds.similarity.cosine',
            query: [1, 2, 3, 4, 5],
            threshold: {
              $gte: 0.5,
              $lte: 0.8,
              $ne: 0.75
            }
          }
        }
      }
    }
  }
}

const r10 = `MATCH (record:__RUSHDB__LABEL__RECORD__ { __RUSHDB__KEY__PROJECT__ID__: $projectId })
ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:DOCUMENT)
OPTIONAL MATCH (record1)--(record2:CHUNK) WHERE ((\`record2\`.\`embedding\` IS NOT NULL AND apoc.convert.fromJsonMap(\`record2\`.\`__RUSHDB__KEY__PROPERTIES__META__\`).\`embedding\` = "vector" AND gds.similarity.cosine(\`record2\`.\`embedding\`, [1,2,3,4,5]) >= 0.5 AND gds.similarity.cosine(\`record2\`.\`embedding\`, [1,2,3,4,5]) <= 0.8 AND gds.similarity.cosine(\`record2\`.\`embedding\`, [1,2,3,4,5]) <> 0.75))
WITH record, record1, record2 WHERE record IS NOT NULL AND (record1 IS NOT NULL AND record2 IS NOT NULL)
RETURN collect(DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}) AS records`

const q11 = {
  where: {
    DOCUMENT: {
      CHUNK: {
        embedding: {
          $vector: {
            fn: 'gds.similarity.cosine',
            query: [1, 2, 3, 4, 5],
            threshold: 0.75
          }
        }
      }
    }
  }
}

const r11 = `MATCH (record:__RUSHDB__LABEL__RECORD__ { __RUSHDB__KEY__PROJECT__ID__: $projectId })
ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:DOCUMENT)
OPTIONAL MATCH (record1)--(record2:CHUNK) WHERE ((\`record2\`.\`embedding\` IS NOT NULL AND apoc.convert.fromJsonMap(\`record2\`.\`__RUSHDB__KEY__PROPERTIES__META__\`).\`embedding\` = "vector" AND gds.similarity.cosine(\`record2\`.\`embedding\`, [1,2,3,4,5]) >= 0.75))
WITH record, record1, record2 WHERE record IS NOT NULL AND (record1 IS NOT NULL AND record2 IS NOT NULL)
RETURN collect(DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}) AS records`

const q12 = {
  where: {
    embedding: {
      $vector: {
        fn: 'gds.similarity.euclidean',
        query: [
          0.0123, -0.4421, 0.9372, 0.1284, -0.3349, 0.7821, -0.2843, 0.1634, 0.4372, -0.219, 0.6612, 0.0841,
          -0.3312, 0.9123, -0.1055, 0.0041, 0.4388, -0.7881, 0.5523, 0.0011, 0.7342, -0.2284, 0.1156, -0.5472,
          0.3328, 0.9811, -0.1112, 0.0045, 0.6233, -0.7
        ],
        threshold: 0.93
      }
    }
  },
  aggregate: {
    similarity: {
      fn: 'gds.similarity.euclidean',
      field: 'embedding',
      query: [
        0.0123, -0.4421, 0.9372, 0.1284, -0.3349, 0.7821, -0.2843, 0.1634, 0.4372, -0.219, 0.6612, 0.0841,
        -0.3312, 0.9123, -0.1055, 0.0041, 0.4388, -0.7881, 0.5523, 0.0011, 0.7342, -0.2284, 0.1156, -0.5472,
        0.3328, 0.9811, -0.1112, 0.0045, 0.6233, -0.7
      ],
      alias: '$record'
    }
  },
  orderBy: 'asc',
  skip: 0,
  limit: 1000
}

const r12 = `MATCH (record:__RUSHDB__LABEL__RECORD__ { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE ((\`record\`.\`embedding\` IS NOT NULL AND apoc.convert.fromJsonMap(\`record\`.\`__RUSHDB__KEY__PROPERTIES__META__\`).\`embedding\` = "vector" AND gds.similarity.euclidean(\`record\`.\`embedding\`, [0.0123,-0.4421,0.9372,0.1284,-0.3349,0.7821,-0.2843,0.1634,0.4372,-0.219,0.6612,0.0841,-0.3312,0.9123,-0.1055,0.0041,0.4388,-0.7881,0.5523,0.0011,0.7342,-0.2284,0.1156,-0.5472,0.3328,0.9811,-0.1112,0.0045,0.6233,-0.7]) <= 0.93)) ORDER BY record.\`__RUSHDB__KEY__ID__\` ASC SKIP 0 LIMIT 1000
WITH record, CASE 
    WHEN \`record\`.\`embedding\` IS NOT NULL AND apoc.convert.fromJsonMap(\`record\`.\`__RUSHDB__KEY__PROPERTIES__META__\`).\`embedding\` = "vector" AND size(\`record\`.\`embedding\`) = size([0.0123,-0.4421,0.9372,0.1284,-0.3349,0.7821,-0.2843,0.1634,0.4372,-0.219,0.6612,0.0841,-0.3312,0.9123,-0.1055,0.0041,0.4388,-0.7881,0.5523,0.0011,0.7342,-0.2284,0.1156,-0.5472,0.3328,0.9811,-0.1112,0.0045,0.6233,-0.7]) 
    THEN gds.similarity.euclidean(\`record\`.\`embedding\`, [0.0123,-0.4421,0.9372,0.1284,-0.3349,0.7821,-0.2843,0.1634,0.4372,-0.219,0.6612,0.0841,-0.3312,0.9123,-0.1055,0.0041,0.4388,-0.7881,0.5523,0.0011,0.7342,-0.2284,0.1156,-0.5472,0.3328,0.9811,-0.1112,0.0045,0.6233,-0.7]) 
    ELSE null 
  END AS \`similarity\`
RETURN collect(DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`similarity\`}) AS records`

const q13 = {
  where: {},
  aggregate: {
    similarity: {
      fn: 'gds.similarity.euclidean',
      field: 'embedding',
      query: [
        0.0123, -0.4421, 0.9372, 0.1284, -0.3349, 0.7821, -0.2843, 0.1634, 0.4372, -0.219, 0.6612, 0.0841,
        -0.3312, 0.9123, -0.1055, 0.0041, 0.4388, -0.7881, 0.5523, 0.0011, 0.7342, -0.2284, 0.1156, -0.5472,
        0.3328, 0.9811, -0.1112, 0.0045, 0.6233, -0.7
      ],
      alias: '$record'
    }
  },
  orderBy: { similarity: 'desc' },
  skip: 0,
  limit: 1000
}

const r13 = `MATCH (record:__RUSHDB__LABEL__RECORD__ { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WITH record, CASE 
    WHEN \`record\`.\`embedding\` IS NOT NULL AND apoc.convert.fromJsonMap(\`record\`.\`__RUSHDB__KEY__PROPERTIES__META__\`).\`embedding\` = "vector" AND size(\`record\`.\`embedding\`) = size([0.0123,-0.4421,0.9372,0.1284,-0.3349,0.7821,-0.2843,0.1634,0.4372,-0.219,0.6612,0.0841,-0.3312,0.9123,-0.1055,0.0041,0.4388,-0.7881,0.5523,0.0011,0.7342,-0.2284,0.1156,-0.5472,0.3328,0.9811,-0.1112,0.0045,0.6233,-0.7]) 
    THEN gds.similarity.euclidean(\`record\`.\`embedding\`, [0.0123,-0.4421,0.9372,0.1284,-0.3349,0.7821,-0.2843,0.1634,0.4372,-0.219,0.6612,0.0841,-0.3312,0.9123,-0.1055,0.0041,0.4388,-0.7881,0.5523,0.0011,0.7342,-0.2284,0.1156,-0.5472,0.3328,0.9811,-0.1112,0.0045,0.6233,-0.7]) 
    ELSE null 
  END AS \`similarity\`
ORDER BY \`similarity\` DESC SKIP 0 LIMIT 1000
RETURN collect(DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`similarity\`}) AS records`

const q14 = {
  labels: ['COMPANY'],
  where: {
    EMPLOYEE: {
      $alias: '$employee'
    }
  },
  aggregate: {
    employeeCount: {
      fn: 'count',
      alias: '$employee'
    }
  },
  orderBy: {
    employeeCount: 'desc'
  },
  skip: 0,
  limit: 1000
}

const r14 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
OPTIONAL MATCH (record)--(record1:EMPLOYEE)
WITH record, record1 WHERE record IS NOT NULL AND record1 IS NOT NULL
WITH record, count(record1) AS \`employeeCount\`
ORDER BY \`employeeCount\` DESC SKIP 0 LIMIT 1000
RETURN collect(DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`employeeCount\`}) AS records`

const q15 = {
  labels: ['COMPANY'],
  where: {
    $and: [
      {
        active: true
      },
      {
        $or: [
          {
            email: {
              $endsWith: '@gmail.com'
            }
          },
          {
            email: {
              $endsWith: '@outlook.com'
            }
          }
        ]
      }
    ]
  },
  skip: 0,
  limit: 1000
}

const r15 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE ((any(value IN record.active WHERE value = true)) AND ((any(value IN record.email WHERE value ENDS WITH "@gmail.com")) OR (any(value IN record.email WHERE value ENDS WITH "@outlook.com")))) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 1000
RETURN collect(DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}) AS records`

const q16 = {
  where: {
    title: {
      $type: 'string'
    }
  },
  skip: 0,
  limit: 1000
}

const r16 = `MATCH (record:__RUSHDB__LABEL__RECORD__ { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE (apoc.convert.fromJsonMap(\`record\`.\`__RUSHDB__KEY__PROPERTIES__META__\`).\`title\` = "string") ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 1000
RETURN collect(DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}) AS records`

const q17 = {
  where: {
    title: {
      $exists: true
    }
  },
  skip: 0,
  limit: 1000
}

const r17 = `MATCH (record:__RUSHDB__LABEL__RECORD__ { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE (record.title IS NOT NULL) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 1000
RETURN collect(DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}) AS records`

const q18 = {
  where: {
    $and: [
      {
        title: {
          $exists: true
        }
      },
      { title: { $type: 'string' } }
    ]
  },
  skip: 0,
  limit: 1000
}

const r18 = `MATCH (record:__RUSHDB__LABEL__RECORD__ { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE ((record.title IS NOT NULL) AND (apoc.convert.fromJsonMap(\`record\`.\`__RUSHDB__KEY__PROPERTIES__META__\`).\`title\` = "string")) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 1000
RETURN collect(DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}) AS records`

const q19 = {
  where: {
    emb: {
      $vector: {
        fn: 'gds.similarity.cosine',
        query: [1, 2, 3, 4, 5],
        threshold: {
          $gte: 0.5,
          $lte: 0.8,
          $ne: 0.75
        }
      }
    }
  }
}

const r19 = `MATCH (record:__RUSHDB__LABEL__RECORD__ { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE ((\`record\`.\`emb\` IS NOT NULL AND apoc.convert.fromJsonMap(\`record\`.\`__RUSHDB__KEY__PROPERTIES__META__\`).\`emb\` = "vector" AND gds.similarity.cosine(\`record\`.\`emb\`, [1,2,3,4,5]) >= 0.5 AND gds.similarity.cosine(\`record\`.\`emb\`, [1,2,3,4,5]) <= 0.8 AND gds.similarity.cosine(\`record\`.\`emb\`, [1,2,3,4,5]) <> 0.75)) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
RETURN collect(DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}) AS records`

describe('build complete query', () => {
  let queryService: EntityQueryService

  beforeEach(() => {
    queryService = new EntityQueryService()
  })

  it('0', () => {
    const result = queryService.findRecords({ searchQuery: q0 })

    expect(result).toEqual(r0)
  })

  it('1', () => {
    const result = queryService.findRecords({ searchQuery: q1 })

    expect(result).toEqual(r1)
  })

  it('2', () => {
    const result = queryService.findRecords({ searchQuery: q2 })

    expect(result).toEqual(r2)
  })

  it('3', () => {
    const result = queryService.findRecords({ searchQuery: q3 })

    expect(result).toEqual(r3)
  })

  it('4', () => {
    const result = queryService.findRecords({ searchQuery: q4 })

    expect(result).toEqual(r4)
  })

  it('5', () => {
    const result = queryService.findRecords({ searchQuery: q5 })

    expect(result).toEqual(r5)
  })

  it('6', () => {
    const result = queryService.findRecords({ searchQuery: q6 })

    expect(result).toEqual(r6)
  })

  it('7', () => {
    const result = queryService.findRecords({ searchQuery: q7 })

    expect(result).toEqual(r7)
  })

  it('8', () => {
    const result = queryService.findRecords({ searchQuery: q8 })

    expect(result).toEqual(r8)
  })

  it('9', () => {
    const result = queryService.findRecords({ searchQuery: q9 })

    expect(result).toEqual(r9)
  })

  it('10', () => {
    const result = queryService.findRecords({ searchQuery: q10 })

    expect(result).toEqual(r10)
  })

  it('11', () => {
    const result = queryService.findRecords({ searchQuery: q11 })

    expect(result).toEqual(r11)
  })

  it('12', () => {
    const result = queryService.findRecords({ searchQuery: q12 })

    expect(result).toEqual(r12)
  })

  it('13', () => {
    const result = queryService.findRecords({ searchQuery: q13 })

    expect(result).toEqual(r13)
  })

  it('14', () => {
    const result = queryService.findRecords({ searchQuery: q14 })

    expect(result).toEqual(r14)
  })

  it('15', () => {
    const result = queryService.findRecords({ searchQuery: q15 })

    expect(result).toEqual(r15)
  })

  it('16', () => {
    const result = queryService.findRecords({ searchQuery: q16 })

    expect(result).toEqual(r16)
  })

  it('17', () => {
    const result = queryService.findRecords({ searchQuery: q17 })

    expect(result).toEqual(r17)
  })

  it('18', () => {
    const result = queryService.findRecords({ searchQuery: q18 })

    expect(result).toEqual(r18)
  })
})
