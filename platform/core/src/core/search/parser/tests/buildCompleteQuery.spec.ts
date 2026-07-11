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
  select: {
    employees: {
      $collect: {
        from: '$employee',
        orderBy: { salary: 'desc' },
        limit: 10
      }
    }
  }
}

const r0 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE ((any(value IN record.\`stage\` WHERE value = "seed")) OR (any(value IN record.\`stage\` WHERE value = "roundA"))) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:__RUSHDB__LABEL__RECORD__:\`EMPLOYEE\`) WHERE (any(value IN record1.\`salary\` WHERE value >= 500000))
WITH record, record1 WHERE record IS NOT NULL AND record1 IS NOT NULL
WITH record, apoc.coll.sortMaps(collect(DISTINCT record1 {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record1) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "salary")[0..10] AS \`employees\`
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`employees\`} as records`

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
  select: {
    departments: {
      $collect: {
        label: 'departments',
        select: {
          projects: {
            $collect: {
              label: 'projects',
              orderBy: { projectName: 'asc', projectId: 'desc' },
              select: {
                employees: {
                  $collect: { label: 'employees', orderBy: { salary: 'desc' }, limit: 3 }
                }
              }
            }
          }
        }
      }
    }
  }
}

const r1 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE (any(value IN record.\`rating\` WHERE value >= 1)) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:__RUSHDB__LABEL__RECORD__:\`departments\`)
OPTIONAL MATCH (record1)--(record2:__RUSHDB__LABEL__RECORD__:\`projects\`)
OPTIONAL MATCH (record2)--(record3:__RUSHDB__LABEL__RECORD__:\`employees\`) WHERE (any(value IN record3.\`salary\` WHERE value >= 499500))
WITH record, record1, record2, record3 WHERE record IS NOT NULL AND (record1 IS NOT NULL AND (record2 IS NOT NULL AND record3 IS NOT NULL))
OPTIONAL MATCH (record)--(sel0:__RUSHDB__LABEL__RECORD__:\`departments\`)
OPTIONAL MATCH (sel0)--(sel1:__RUSHDB__LABEL__RECORD__:\`projects\`)
OPTIONAL MATCH (sel1)--(sel2:__RUSHDB__LABEL__RECORD__:\`employees\`)
WITH record, sel0, sel1, apoc.coll.sortMaps(collect(DISTINCT sel2 {.*, __RUSHDB__KEY__LABEL__: [label IN labels(sel2) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "salary")[0..3] AS \`employees\`
WITH record, sel0, apoc.coll.sortMaps(collect(DISTINCT sel1 {\`employees\`, __RUSHDB__KEY__LABEL__: [label IN labels(sel1) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "^projectName")[0..100] AS \`projects\`
WITH record, apoc.coll.sortMaps(collect(DISTINCT sel0 {\`projects\`, __RUSHDB__KEY__LABEL__: [label IN labels(sel0) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "__RUSHDB__KEY__ID__")[0..100] AS \`departments\`
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`departments\`} as records`

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
  select: {
    companyName: '$record.name',
    employeesCount: { $count: '$employee' },
    totalWage: { $sum: '$employee.salary' },
    avgSalary: { $avg: '$employee.salary', $precision: 0 },
    minSalary: { $min: '$employee.salary' },
    maxSalary: { $max: '$employee.salary' }
  }
}

const r2 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE (any(value IN record.\`rating\` WHERE value >= 1)) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:__RUSHDB__LABEL__RECORD__:\`departments\`)
OPTIONAL MATCH (record1)--(record2:__RUSHDB__LABEL__RECORD__:\`projects\`)
OPTIONAL MATCH (record2)--(record3:__RUSHDB__LABEL__RECORD__:\`employees\`) WHERE (any(value IN record3.\`salary\` WHERE value >= 499500))
WITH record, record1, record2, record3 WHERE record IS NOT NULL AND (record1 IS NOT NULL AND (record2 IS NOT NULL AND record3 IS NOT NULL))
WITH record, count(DISTINCT record3) AS \`employeesCount\`, sum(record3.\`salary\`) AS \`totalWage\`, toInteger(avg(record3.\`salary\`)) AS \`avgSalary\`, min(record3.\`salary\`) AS \`minSalary\`, max(record3.\`salary\`) AS \`maxSalary\`
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`companyName\`: record.\`name\`, \`employeesCount\`, \`totalWage\`, \`avgSalary\`, \`minSalary\`, \`maxSalary\`} as records`

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
  select: {
    companyName: '$record.name',
    employeesCount: { $count: '$employee' },
    totalWage: { $sum: '$employee.salary' },
    avgSalary: { $avg: '$employee.salary', $precision: 0 },
    minSalary: { $min: '$employee.salary' },
    maxSalary: { $max: '$employee.salary' }
  }
}

const r3 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE (any(value IN record.\`tag\` WHERE value = "top-sellers")) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)<-[:AUTHORED]-(record1:__RUSHDB__LABEL__RECORD__:\`AUTHOR\`) WHERE (((any(value IN record1.\`name\` WHERE value STARTS WITH "Jack") AND any(value IN record1.\`name\` WHERE value ENDS WITH "Rooney")) OR any(value IN record1.\`name\` WHERE apoc.convert.fromJsonMap(record1.\`__RUSHDB__KEY__PROPERTIES__META__\`).\`name\` = "datetime" AND datetime(value) = datetime({year: 1984}))))
OPTIONAL MATCH (record)--(record2:__RUSHDB__LABEL__RECORD__:\`POST\`) WHERE (any(value IN record2.\`created\` WHERE apoc.convert.fromJsonMap(record2.\`__RUSHDB__KEY__PROPERTIES__META__\`).\`created\` = "datetime" AND datetime(value) = datetime({year: 2011, month: 11, day: 11}))) AND (((any(value IN record2.\`rating\` WHERE value > 4.5) AND any(value IN record2.\`rating\` WHERE value < 6)) OR any(value IN record2.\`rating\` WHERE value <> 3) OR (NOT(any(value IN record2.\`rating\` WHERE value >= 4))))) AND (any(value IN record2.\`title\` WHERE value <> "Forest"))
OPTIONAL MATCH (record2)-[:COMMENT_TO_POST]->(record3:__RUSHDB__LABEL__RECORD__:\`COMMENT\`) WHERE (any(value IN record3.\`authoredBy\` WHERE value =~ "(?i).*Sam.*"))
OPTIONAL MATCH (record2)--(record4:__RUSHDB__LABEL__RECORD__:\`CAR\`) WHERE (any(value IN record4.\`color\` WHERE value = "red"))
OPTIONAL MATCH (record4)--(record5:__RUSHDB__LABEL__RECORD__:\`SPOUSE\`) WHERE (any(value IN record5.\`gender\` WHERE value = "male"))
OPTIONAL MATCH (record2)--(record6:__RUSHDB__LABEL__RECORD__:\`JOB\`) WHERE (any(value IN record6.\`title\` WHERE value = "Manager"))
OPTIONAL MATCH (record)--(record7:__RUSHDB__LABEL__RECORD__:\`departments\`)
OPTIONAL MATCH (record7)--(record8:__RUSHDB__LABEL__RECORD__:\`projects\`)
OPTIONAL MATCH (record8)--(record9:__RUSHDB__LABEL__RECORD__:\`employees\`) WHERE (any(value IN record9.\`salary\` WHERE value >= 499500))
WITH record, record1, record2, record3, record4, record5, record6, record7, record8, record9 WHERE record IS NOT NULL AND record1 IS NOT NULL AND (record2 IS NOT NULL AND record3 IS NOT NULL) AND (record4 IS NOT NULL AND (record5 IS NOT NULL AND record6 IS NOT NULL))
WITH record, count(DISTINCT record9) AS \`employeesCount\`, sum(record9.\`salary\`) AS \`totalWage\`, toInteger(avg(record9.\`salary\`)) AS \`avgSalary\`, min(record9.\`salary\`) AS \`minSalary\`, max(record9.\`salary\`) AS \`maxSalary\`
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`companyName\`: record.\`name\`, \`employeesCount\`, \`totalWage\`, \`avgSalary\`, \`minSalary\`, \`maxSalary\`} as records`

const q4 = {
  labels: ['COMPANY'],
  where: {
    stage: { $or: ['seed', 'roundA'] }
  }
}

const r4 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE ((any(value IN record.\`stage\` WHERE value = "seed") OR any(value IN record.\`stage\` WHERE value = "roundA"))) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]} AS records`

const q5 = {
  labels: ['COMPANY'],
  where: {
    $or: [{ stage: 'seed' }, { stage: 'roundA' }]
  }
}

const r5 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE ((any(value IN record.\`stage\` WHERE value = "seed")) OR (any(value IN record.\`stage\` WHERE value = "roundA"))) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]} AS records`

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
WHERE (((any(value IN record.\`__RUSHDB__KEY__ID__\` WHERE value = "1234567890") OR any(value IN record.\`__RUSHDB__KEY__ID__\` WHERE value = "0987654321"))) AND (any(value IN record.\`name\` WHERE value = "alex"))) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:__RUSHDB__LABEL__RECORD__:\`USER\`) WHERE (any(value IN record1.\`__RUSHDB__KEY__ID__\` WHERE value IN ["1234567890", "0987654321"]))
WITH record, record1 WHERE record IS NOT NULL AND record1 IS NOT NULL
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]} AS records`

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
  select: {
    tags: {
      $collect: { from: '$department', select: { tags: '$department.tags' } }
    }
  }
}

const r7 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE (any(value IN record.\`rating\` WHERE value >= 1)) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:__RUSHDB__LABEL__RECORD__:\`departments\`)
WITH record, record1 WHERE record IS NOT NULL AND record1 IS NOT NULL
WITH record, apoc.coll.sortMaps(collect(DISTINCT record1 {tags: record1.\`tags\`, __RUSHDB__KEY__LABEL__: [label IN labels(record1) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "__RUSHDB__KEY__ID__")[0..100] AS \`tags\`
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`tags\`} as records`

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
  select: {
    departmentId: '$department.__id'
  }
}

const r8 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:__RUSHDB__LABEL__RECORD__:\`departments\`)
WITH record, record1 WHERE record IS NOT NULL AND record1 IS NOT NULL
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`departmentId\`: record1.\`__RUSHDB__KEY__ID__\`} as records`

const q13 = {
  where: {},
  select: { embedding: '$record.embedding' },
  skip: 0,
  limit: 1000
}

const r13 = `MATCH (record:__RUSHDB__LABEL__RECORD__ { __RUSHDB__KEY__PROJECT__ID__: $projectId })
ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 1000
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`embedding\`: record.\`embedding\`} as records`

const q14 = {
  labels: ['COMPANY'],
  where: {
    EMPLOYEE: {
      $alias: '$employee'
    }
  },
  select: {
    employeeCount: { $count: '$employee' }
  },
  orderBy: {
    employeeCount: 'desc'
  },
  skip: 0,
  limit: 1000
}

const r14 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
OPTIONAL MATCH (record)--(record1:__RUSHDB__LABEL__RECORD__:\`EMPLOYEE\`)
WITH record, record1 WHERE record IS NOT NULL AND record1 IS NOT NULL
WITH record, count(DISTINCT record1) AS \`employeeCount\`
ORDER BY \`employeeCount\` DESC SKIP 0 LIMIT 1000
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`employeeCount\`} as records`

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
WHERE ((any(value IN record.\`active\` WHERE value = true)) AND ((any(value IN record.\`email\` WHERE value ENDS WITH "@gmail.com")) OR (any(value IN record.\`email\` WHERE value ENDS WITH "@outlook.com")))) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 1000
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]} AS records`

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
WHERE (apoc.convert.fromJsonMap(record.\`__RUSHDB__KEY__PROPERTIES__META__\`).\`title\` = "string") ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 1000
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]} AS records`

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
WHERE (record.\`title\` IS NOT NULL) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 1000
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]} AS records`

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
WHERE ((record.\`title\` IS NOT NULL) AND (apoc.convert.fromJsonMap(record.\`__RUSHDB__KEY__PROPERTIES__META__\`).\`title\` = "string")) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 1000
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]} AS records`

const q19 = {
  where: {
    HS_APPOINTMENT: {
      $alias: '$appointment'
    }
  },
  select: {
    count: { $count: '$record' },
    avgAmount: { $avg: '$record.amount' }
  },
  groupBy: ['$record.dealstage'],
  orderBy: {
    count: 'desc'
  },
  skip: 0,
  limit: 1000,
  labels: ['HS_DEAL']
}

const r19 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`HS_DEAL\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
OPTIONAL MATCH (record)--(record1:__RUSHDB__LABEL__RECORD__:\`HS_APPOINTMENT\`)
WITH record, record1 WHERE record IS NOT NULL AND record1 IS NOT NULL
WITH count(DISTINCT record) AS \`count\`, avg(record.\`amount\`) AS \`avgAmount\`, record.\`dealstage\` AS \`dealstage\`
ORDER BY \`count\` DESC SKIP 0 LIMIT 1000
RETURN {\`count\`:\`count\`, \`avgAmount\`:\`avgAmount\`, \`dealstage\`:\`dealstage\`} as records`

const q20 = {
  where: {
    dealstage: 'closedwon',
    HS_APPOINTMENT: {
      $alias: '$appointment'
    }
  },
  select: {
    count: { $count: '$record' },
    avgAmount: { $sum: '$record.amount' }
  },
  groupBy: ['$appointment.hs_meeting_location'],
  orderBy: {
    count: 'desc'
  },
  skip: 0,
  limit: 1000,
  labels: ['HS_DEAL']
}

const r20 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`HS_DEAL\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE (any(value IN record.\`dealstage\` WHERE value = "closedwon"))
OPTIONAL MATCH (record)--(record1:__RUSHDB__LABEL__RECORD__:\`HS_APPOINTMENT\`)
WITH record, record1 WHERE record IS NOT NULL AND record1 IS NOT NULL
WITH count(DISTINCT record) AS \`count\`, sum(record.\`amount\`) AS \`avgAmount\`, record1.\`hs_meeting_location\` AS \`hs_meeting_location\`
ORDER BY \`count\` DESC SKIP 0 LIMIT 1000
RETURN {\`count\`:\`count\`, \`avgAmount\`:\`avgAmount\`, \`hs_meeting_location\`:\`hs_meeting_location\`} as records`

const q21 = {
  labels: ['DEPARTMENT'],
  where: {
    PROJECT: {
      $alias: '$project'
    }
  },
  select: {
    projects: { $collect: { from: '$project', select: { name: '$project.name' } } },
    projectsCount: { $count: '$project' }
  },
  groupBy: ['$record.name'],
  orderBy: {
    projectsCount: 'desc'
  },
  skip: 0,
  limit: 1000
}

const r21 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`DEPARTMENT\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
OPTIONAL MATCH (record)--(record1:__RUSHDB__LABEL__RECORD__:\`PROJECT\`)
WITH record, record1 WHERE record IS NOT NULL AND record1 IS NOT NULL
WITH apoc.coll.sortMaps(collect(DISTINCT record1 {name: record1.\`name\`, __RUSHDB__KEY__LABEL__: [label IN labels(record1) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "__RUSHDB__KEY__ID__")[0..100] AS \`projects\`, count(DISTINCT record1) AS \`projectsCount\`, record.\`name\` AS \`name\`
ORDER BY \`projectsCount\` DESC SKIP 0 LIMIT 1000
RETURN {\`projects\`:\`projects\`, \`projectsCount\`:\`projectsCount\`, \`name\`:\`name\`} as records`

const q22 = {
  labels: ['PROJECT'],
  select: {
    count: { $count: '$record' }
  },
  groupBy: ['$record.active'],
  orderBy: {
    count: 'desc'
  },
  skip: 0,
  limit: 1000
}

const r22 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`PROJECT\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WITH count(DISTINCT record) AS \`count\`, record.\`active\` AS \`active\`
ORDER BY \`count\` DESC SKIP 0 LIMIT 1000
RETURN {\`count\`:\`count\`, \`active\`:\`active\`} as records`

const q23 = {
  labels: ['HS_DEAL'],
  select: {
    totalAmount: { $sum: '$record.amount' }
  },
  groupBy: ['totalAmount']
}

const r23 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`HS_DEAL\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
WITH sum(record.\`amount\`) AS \`totalAmount\`
RETURN {\`totalAmount\`:\`totalAmount\`} as records`

const q24 = {
  select: {
    byYear: { $timeBucket: { field: '$record.dob', unit: 'year' } },
    count: { $count: '$record' }
  },
  groupBy: ['byYear'],
  orderBy: {
    byYear: 'asc'
  },
  skip: 0,
  limit: 100,
  labels: ['EMPLOYEE']
}

const r24 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`EMPLOYEE\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WITH CASE WHEN any(t IN ['datetime', 'date'] WHERE record.\`dob\` IS NOT NULL) THEN datetime({year: record.\`dob\`.year}).epochMillis ELSE null END AS \`byYear\`, count(DISTINCT record) AS \`count\`
ORDER BY \`byYear\` ASC SKIP 0 LIMIT 100
RETURN {\`count\`:\`count\`, \`byYear\`:\`byYear\`} as records`

const q25 = {
  select: {
    byYear: { $timeBucket: { field: '$record.dob', unit: 'week' } },
    count: { $count: '$record' }
  },
  groupBy: ['byYear'],
  orderBy: {
    byYear: 'asc'
  },
  skip: 0,
  limit: 1000,
  labels: ['EMPLOYEE']
}

const r25 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`EMPLOYEE\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WITH CASE WHEN any(t IN ['datetime', 'date'] WHERE record.\`dob\` IS NOT NULL) THEN datetime({year: record.\`dob\`.year, week: record.\`dob\`.week}).epochMillis ELSE null END AS \`byYear\`, count(DISTINCT record) AS \`count\`
ORDER BY \`byYear\` ASC SKIP 0 LIMIT 1000
RETURN {\`count\`:\`count\`, \`byYear\`:\`byYear\`} as records`

const q26 = {
  select: {
    byYear: { $timeBucket: { field: '$record.dob', unit: 'months', size: 6 } },
    count: { $count: '$record' }
  },
  groupBy: ['byYear'],
  orderBy: {
    byYear: 'asc'
  },
  skip: 0,
  limit: 1000,
  labels: ['EMPLOYEE']
}

const r26 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`EMPLOYEE\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WITH CASE WHEN any(t IN ['datetime', 'date'] WHERE record.\`dob\` IS NOT NULL) THEN datetime({year: record.\`dob\`.year, month: (toInteger((record.\`dob\`.month - 1) / 6) * 6) + 1}).epochMillis ELSE null END AS \`byYear\`, count(DISTINCT record) AS \`count\`
ORDER BY \`byYear\` ASC SKIP 0 LIMIT 1000
RETURN {\`count\`:\`count\`, \`byYear\`:\`byYear\`} as records`

const q27 = {
  select: {
    byYear: { $timeBucket: { field: '$record.dob', unit: 'quarter' } },
    count: { $count: '$record' }
  },
  groupBy: ['byYear'],
  orderBy: {
    byYear: 'asc'
  },
  skip: 0,
  limit: 1000,
  labels: ['EMPLOYEE']
}

const r27 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`EMPLOYEE\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WITH CASE WHEN any(t IN ['datetime', 'date'] WHERE record.\`dob\` IS NOT NULL) THEN datetime({year: record.\`dob\`.year, month: (toInteger((record.\`dob\`.month - 1) / 3) * 3) + 1}).epochMillis ELSE null END AS \`byYear\`, count(DISTINCT record) AS \`count\`
ORDER BY \`byYear\` ASC SKIP 0 LIMIT 1000
RETURN {\`count\`:\`count\`, \`byYear\`:\`byYear\`} as records`

const q28 = {
  select: {
    byHour: { $timeBucket: { field: '$record.dob', unit: 'hour' } },
    byMinute: { $timeBucket: { field: '$record.dob', unit: 'minute' } },
    bySecond: { $timeBucket: { field: '$record.dob', unit: 'second' } },
    count: { $count: '$record' }
  },
  groupBy: ['byHour', 'byMinute', 'bySecond'],
  orderBy: { byHour: 'asc' },
  skip: 0,
  limit: 1000,
  labels: ['EMPLOYEE']
}

const r28 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`EMPLOYEE\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WITH CASE WHEN any(t IN ['datetime', 'date'] WHERE record.\`dob\` IS NOT NULL) THEN datetime({year: record.\`dob\`.year, month: record.\`dob\`.month, day: record.\`dob\`.day, hour: record.\`dob\`.hour}).epochMillis ELSE null END AS \`byHour\`, CASE WHEN any(t IN ['datetime', 'date'] WHERE record.\`dob\` IS NOT NULL) THEN datetime({year: record.\`dob\`.year, month: record.\`dob\`.month, day: record.\`dob\`.day, hour: record.\`dob\`.hour, minute: record.\`dob\`.minute}).epochMillis ELSE null END AS \`byMinute\`, CASE WHEN any(t IN ['datetime', 'date'] WHERE record.\`dob\` IS NOT NULL) THEN datetime({year: record.\`dob\`.year, month: record.\`dob\`.month, day: record.\`dob\`.day, hour: record.\`dob\`.hour, minute: record.\`dob\`.minute, second: record.\`dob\`.second}).epochMillis ELSE null END AS \`bySecond\`, count(DISTINCT record) AS \`count\`
ORDER BY \`byHour\` ASC SKIP 0 LIMIT 1000
RETURN {\`count\`:\`count\`, \`byHour\`:\`byHour\`, \`byMinute\`:\`byMinute\`, \`bySecond\`:\`bySecond\`} as records`

const q29 = {
  select: {
    by6Hours: { $timeBucket: { field: '$record.dob', unit: 'hours', size: 6 } },
    by15Min: { $timeBucket: { field: '$record.dob', unit: 'minutes', size: 15 } },
    by30Sec: { $timeBucket: { field: '$record.dob', unit: 'seconds', size: 30 } },
    count: { $count: '$record' }
  },
  groupBy: ['by6Hours', 'by15Min', 'by30Sec'],
  orderBy: { by6Hours: 'asc' },
  skip: 0,
  limit: 1000,
  labels: ['EMPLOYEE']
}

const r29 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`EMPLOYEE\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WITH CASE WHEN any(t IN ['datetime', 'date'] WHERE record.\`dob\` IS NOT NULL) THEN datetime({year: record.\`dob\`.year, month: record.\`dob\`.month, day: record.\`dob\`.day, hour: (toInteger(record.\`dob\`.hour / 6) * 6)}).epochMillis ELSE null END AS \`by6Hours\`, CASE WHEN any(t IN ['datetime', 'date'] WHERE record.\`dob\` IS NOT NULL) THEN datetime({year: record.\`dob\`.year, month: record.\`dob\`.month, day: record.\`dob\`.day, hour: record.\`dob\`.hour, minute: (toInteger(record.\`dob\`.minute / 15) * 15)}).epochMillis ELSE null END AS \`by15Min\`, CASE WHEN any(t IN ['datetime', 'date'] WHERE record.\`dob\` IS NOT NULL) THEN datetime({year: record.\`dob\`.year, month: record.\`dob\`.month, day: record.\`dob\`.day, hour: record.\`dob\`.hour, minute: record.\`dob\`.minute, second: (toInteger(record.\`dob\`.second / 30) * 30)}).epochMillis ELSE null END AS \`by30Sec\`, count(DISTINCT record) AS \`count\`
ORDER BY \`by6Hours\` ASC SKIP 0 LIMIT 1000
RETURN {\`count\`:\`count\`, \`by6Hours\`:\`by6Hours\`, \`by15Min\`:\`by15Min\`, \`by30Sec\`:\`by30Sec\`} as records`

const q30 = {
  select: {
    by5Years: { $timeBucket: { field: '$record.dob', unit: 'years', size: 5 } },
    count: { $count: '$record' }
  },
  groupBy: ['by5Years'],
  orderBy: { by5Years: 'asc' },
  skip: 0,
  limit: 1000,
  labels: ['EMPLOYEE']
}

const r30 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`EMPLOYEE\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WITH CASE WHEN any(t IN ['datetime', 'date'] WHERE record.\`dob\` IS NOT NULL) THEN datetime({year: (toInteger(record.\`dob\`.year / 5) * 5)}).epochMillis ELSE null END AS \`by5Years\`, count(DISTINCT record) AS \`count\`
ORDER BY \`by5Years\` ASC SKIP 0 LIMIT 1000
RETURN {\`count\`:\`count\`, \`by5Years\`:\`by5Years\`} as records`

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

  it('19', () => {
    const result = queryService.findRecords({ searchQuery: q19 })

    expect(result).toEqual(r19)
  })

  it('20', () => {
    const result = queryService.findRecords({ searchQuery: q20 })

    expect(result).toEqual(r20)
  })

  it('21', () => {
    const result = queryService.findRecords({ searchQuery: q21 })

    expect(result).toEqual(r21)
  })

  it('22', () => {
    const result = queryService.findRecords({ searchQuery: q22 })

    expect(result).toEqual(r22)
  })

  it('23', () => {
    const result = queryService.findRecords({ searchQuery: q23 })

    expect(result).toEqual(r23)
  })

  it('24', () => {
    const result = queryService.findRecords({ searchQuery: q24 })

    expect(result).toEqual(r24)
  })

  it('25', () => {
    const result = queryService.findRecords({ searchQuery: q25 })

    expect(result).toEqual(r25)
  })

  it('26', () => {
    const result = queryService.findRecords({ searchQuery: q26 })

    expect(result).toEqual(r26)
  })

  it('27', () => {
    const result = queryService.findRecords({ searchQuery: q27 })

    expect(result).toEqual(r27)
  })

  it('28 - timeBucket hour/minute/second', () => {
    const result = queryService.findRecords({ searchQuery: q28 })

    expect(result).toEqual(r28)
  })

  it('29 - timeBucket hours/minutes/seconds with size', () => {
    const result = queryService.findRecords({ searchQuery: q29 })
    expect(result).toEqual(r29)
  })

  it('30 - timeBucket years with size', () => {
    const result = queryService.findRecords({ searchQuery: q30 })

    expect(result).toEqual(r30)
  })
})

// ─────────────────────────────────────────────────────────────────────────────
// select expression API — new-style queries
// Each test verifies the Cypher output of the `select` key, including
// ($ref, $collect with projection, $timeBucket, math operators).
// ─────────────────────────────────────────────────────────────────────────────

// ── Inputs ───────────────────────────────────────────────────────────────────

// Equivalent of q2 — multi-aggregation + inline field ref, no groupBy
const q31 = {
  where: {
    rating: { $gte: 1 },
    departments: {
      $alias: '$department',
      projects: {
        $alias: '$project',
        employees: { $alias: '$employee', salary: { $gte: 499500 } }
      }
    }
  },
  orderBy: {},
  skip: 0,
  limit: 100,
  labels: ['COMPANY'],
  select: {
    companyName: '$record.name',
    employeesCount: { $count: '$employee' },
    totalWage: { $sum: '$employee.salary' },
    avgSalary: { $avg: '$employee.salary', $precision: 0 },
    minSalary: { $min: '$employee.salary' },
    maxSalary: { $max: '$employee.salary' }
  }
}

// Equivalent of q14 — $count with sort by aggregated field
const q32 = {
  labels: ['COMPANY'],
  where: { EMPLOYEE: { $alias: '$employee' } },
  select: { employeeCount: { $count: '$employee' } },
  orderBy: { employeeCount: 'desc' },
  skip: 0,
  limit: 1000
}

// Equivalent of q0 — $collect full records (no field projection)
const q33 = {
  labels: ['COMPANY'],
  where: {
    $or: [{ stage: 'seed' }, { stage: 'roundA' }],
    EMPLOYEE: { $alias: '$employee', salary: { $gte: 500_000 } }
  },
  select: {
    employees: { $collect: { from: '$employee', orderBy: { salary: 'desc' }, limit: 10 } }
  }
}

// Equivalent of q22 — groupBy with dimension field, COUNT(*)
const q34 = {
  labels: ['PROJECT'],
  select: { count: { $count: '*' } },
  groupBy: ['$record.active'],
  orderBy: { count: 'desc' },
  skip: 0,
  limit: 1000
}

// Equivalent of q23 — self-groupBy on aggregated key
const q35 = {
  labels: ['HS_DEAL'],
  select: { totalAmount: { $sum: '$record.amount' } },
  groupBy: ['totalAmount']
}

// Equivalent of q19 — groupBy with root-record dimension, COUNT(*) + AVG
const q36 = {
  where: { HS_APPOINTMENT: { $alias: '$appointment' } },
  select: {
    count: { $count: '*' },
    avgAmount: { $avg: '$record.amount' }
  },
  groupBy: ['$record.dealstage'],
  orderBy: { count: 'desc' },
  skip: 0,
  limit: 1000,
  labels: ['HS_DEAL']
}

// Equivalent of q20 — groupBy dimension from a related-record alias, COUNT(*) + SUM
const q37 = {
  where: {
    dealstage: 'closedwon',
    HS_APPOINTMENT: { $alias: '$appointment' }
  },
  select: {
    count: { $count: '*' },
    avgAmount: { $sum: '$record.amount' }
  },
  groupBy: ['$appointment.hs_meeting_location'],
  orderBy: { count: 'desc' },
  skip: 0,
  limit: 1000,
  labels: ['HS_DEAL']
}

// NEW — $ref cross-expression: compute derived metric from two aggregation keys
const q38 = {
  labels: ['ORDER'],
  where: { CUSTOMER: { $alias: '$customer' } },
  select: {
    revenue: { $sum: '$record.amount' },
    orders: { $count: '*' },
    avgOrderValue: { $divide: [{ $ref: 'revenue' }, { $ref: 'orders' }] }
  }
}

// NEW — $collect with explicit field projection
const q39 = {
  labels: ['COMPANY'],
  where: { EMPLOYEE: { $alias: '$employee', salary: { $gte: 50000 } } },
  select: {
    companyName: '$record.name',
    employees: {
      $collect: {
        from: '$employee',
        select: { name: '$employee.name', salary: '$employee.salary' },
        orderBy: { salary: 'desc' },
        limit: 5
      }
    }
  }
}

// NEW — $timeBucket 'year' + COUNT(*), self-groupBy
const q40 = {
  labels: ['EMPLOYEE'],
  select: {
    byYear: { $timeBucket: { field: '$record.dob', unit: 'year' } },
    count: { $count: '*' }
  },
  groupBy: ['byYear'],
  orderBy: { byYear: 'asc' },
  skip: 0,
  limit: 100
}

// NEW — $timeBucket 'month' + $sum, self-groupBy
const q41 = {
  labels: ['ORDER'],
  select: {
    month: { $timeBucket: { field: '$record.issuedAt', unit: 'month' } },
    revenue: { $sum: '$record.amount' }
  },
  groupBy: ['month'],
  orderBy: { month: 'asc' }
}

// NEW — $multiply math operator
const q42 = {
  labels: ['PRODUCT'],
  where: { INVENTORY: { $alias: '$inv' } },
  select: {
    product: '$record.name',
    totalValue: { $multiply: ['$inv.quantity', '$inv.unitPrice'] }
  }
}

const q48 = {
  labels: ['EMPLOYEE'],
  select: {
    name: '$record.name',
    salary: '$record.salary'
  },
  orderBy: {
    salary: 'desc'
  }
}

const q49 = {
  labels: ['EMPLOYEE'],
  select: {
    name: '$record.name'
  },
  orderBy: {
    salary: 'desc'
  }
}

// ── Expected outputs ─────────────────────────────────────────────────────────

// q31 → identical Cypher to r2 (same logic, different syntax)
const r31 = r2

// q32 → identical Cypher to r14
const r32 = r14

// q33 → identical Cypher to r0
const r33 = r0

// q34 → identical Cypher to r22
const r34 = r22

// q35 → identical Cypher to r23
const r35 = r23

// q36 → identical Cypher to r19
const r36 = r19

// q37 → identical Cypher to r20
const r37 = r20

const r38 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`ORDER\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:__RUSHDB__LABEL__RECORD__:\`CUSTOMER\`)
WITH record, record1 WHERE record IS NOT NULL AND record1 IS NOT NULL
WITH record, sum(record.\`amount\`) AS \`revenue\`, count(DISTINCT record) AS \`orders\`
WITH record, \`revenue\`, \`orders\`, (\`revenue\` / \`orders\`) AS \`avgOrderValue\`
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`revenue\`, \`orders\`, \`avgOrderValue\`} as records`

const r39 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:__RUSHDB__LABEL__RECORD__:\`EMPLOYEE\`) WHERE (any(value IN record1.\`salary\` WHERE value >= 50000))
WITH record, record1 WHERE record IS NOT NULL AND record1 IS NOT NULL
WITH record, apoc.coll.sortMaps(collect(DISTINCT record1 {name: record1.\`name\`, salary: record1.\`salary\`, __RUSHDB__KEY__LABEL__: [label IN labels(record1) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "salary")[0..5] AS \`employees\`
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`companyName\`: record.\`name\`, \`employees\`} as records`

const r40 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`EMPLOYEE\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WITH CASE WHEN any(t IN ['datetime', 'date'] WHERE record.\`dob\` IS NOT NULL) THEN datetime({year: record.\`dob\`.year}).epochMillis ELSE null END AS \`byYear\`, count(DISTINCT record) AS \`count\`
ORDER BY \`byYear\` ASC SKIP 0 LIMIT 100
RETURN {\`count\`:\`count\`, \`byYear\`:\`byYear\`} as records`

const r41 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`ORDER\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WITH CASE WHEN any(t IN ['datetime', 'date'] WHERE record.\`issuedAt\` IS NOT NULL) THEN datetime({year: record.\`issuedAt\`.year, month: record.\`issuedAt\`.month}).epochMillis ELSE null END AS \`month\`, sum(record.\`amount\`) AS \`revenue\`
ORDER BY \`month\` ASC SKIP 0 LIMIT 100
RETURN {\`revenue\`:\`revenue\`, \`month\`:\`month\`} as records`

const r42 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`PRODUCT\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:__RUSHDB__LABEL__RECORD__:\`INVENTORY\`)
WITH record, record1 WHERE record IS NOT NULL AND record1 IS NOT NULL
WITH record, (record1.\`quantity\` * record1.\`unitPrice\`) AS \`totalValue\`
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`product\`: record.\`name\`, \`totalValue\`} as records`

const r48 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`EMPLOYEE\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
ORDER BY record.\`salary\` DESC SKIP 0 LIMIT 100
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`name\`: record.\`name\`, \`salary\`: record.\`salary\`} as records`

const r49 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`EMPLOYEE\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
ORDER BY record.\`salary\` DESC SKIP 0 LIMIT 100
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`name\`: record.\`name\`} as records`

// ── Tests ────────────────────────────────────────────────────────────────────

describe('select expression API', () => {
  let queryService: EntityQueryService

  beforeEach(() => {
    queryService = new EntityQueryService()
  })

  it('31 - multi-aggregation + field ref, no groupBy', () => {
    expect(queryService.findRecords({ searchQuery: q31 })).toEqual(r31)
  })

  it('32 - $count with sort by aggregated field', () => {
    expect(queryService.findRecords({ searchQuery: q32 })).toEqual(r32)
  })

  it('33 - $collect full records', () => {
    expect(queryService.findRecords({ searchQuery: q33 })).toEqual(r33)
  })

  it('34 - groupBy with root-record dimension field', () => {
    expect(queryService.findRecords({ searchQuery: q34 })).toEqual(r34)
  })

  it('35 - self-groupBy on aggregated key', () => {
    expect(queryService.findRecords({ searchQuery: q35 })).toEqual(r35)
  })

  it('36 - groupBy with root-record dimension, COUNT(*) + AVG', () => {
    expect(queryService.findRecords({ searchQuery: q36 })).toEqual(r36)
  })

  it('37 - groupBy dimension from related alias, COUNT(*) + SUM', () => {
    expect(queryService.findRecords({ searchQuery: q37 })).toEqual(r37)
  })

  it('38 - $ref cross-expression: derived metric (revenue / orders)', () => {
    expect(queryService.findRecords({ searchQuery: q38 })).toEqual(r38)
  })

  it('39 - $collect with custom field projection', () => {
    expect(queryService.findRecords({ searchQuery: q39 })).toEqual(r39)
  })

  it('40 - $timeBucket year + COUNT(*), self-groupBy', () => {
    expect(queryService.findRecords({ searchQuery: q40 })).toEqual(r40)
  })

  it('41 - $timeBucket month + $sum, self-groupBy', () => {
    expect(queryService.findRecords({ searchQuery: q41 })).toEqual(r41)
  })

  it('42 - $multiply math operator', () => {
    expect(queryService.findRecords({ searchQuery: q42 })).toEqual(r42)
  })

  it('48 - orders by raw record field when orderBy matches a simple select projection', () => {
    expect(queryService.findRecords({ searchQuery: q48 })).toEqual(r48)
  })

  it('49 - orders by raw record field when orderBy field is not selected', () => {
    expect(queryService.findRecords({ searchQuery: q49 })).toEqual(r49)
  })
})

describe('relationship search query API', () => {
  let queryService: EntityQueryService

  beforeEach(() => {
    queryService = new EntityQueryService()
  })

  it('filters relationship edges in top-level where and endpoint records in source/target', () => {
    const query = queryService.getRecordRelations({
      searchQuery: {
        source: { labels: ['USER'], where: { status: 'active' } },
        target: { labels: ['ORDER'] },
        where: { type: 'ORDERED', confidence: { $gte: 0.8 } },
        limit: 25
      }
    })

    expect(query).toContain('MATCH (source:__RUSHDB__LABEL__RECORD__')
    expect(query).toContain('-[rel]->')
    expect(query).toContain('any(label IN labels(source) WHERE label IN ["USER"])')
    expect(query).toContain('any(value IN source.`status` WHERE value = "active")')
    expect(query).toContain('any(label IN labels(target) WHERE label IN ["ORDER"])')
    expect(query).toContain('type(rel) = "ORDERED"')
    expect(query).toContain('rel.`confidence`')
    expect(query).toContain('SKIP 0 LIMIT 25')
  })

  it('keeps record-local relationship listing bidirectional unless direction is explicit', () => {
    const query = queryService.getRecordRelations({ id: 'record-id' })

    expect(query).toContain('(source:__RUSHDB__LABEL__RECORD__')
    expect(query).toContain('-[rel]-(target:__RUSHDB__LABEL__RECORD__')
    expect(query).toContain('any(value IN source.`__RUSHDB__KEY__ID__` WHERE value = "record-id")')
  })

  it('supports incoming direction filters for relationship search', () => {
    const query = queryService.getRecordRelations({
      searchQuery: {
        source: { where: { $id: 'record-id' } },
        where: { direction: 'in', type: 'MENTIONS' }
      }
    })

    expect(query).toContain('<-[rel]-')
    expect(query).toContain('type(rel) = "MENTIONS"')
  })
})

// ─────────────────────────────────────────────────────────────────────────────
// label-based $collect — inline traversal without pre-declared $alias
// Tests 43–46 cover the new unified select path: $collect.label + $self + $collect.where
// ─────────────────────────────────────────────────────────────────────────────

const q43 = {
  labels: ['COMPANY'],
  select: {
    name: '$record.name',
    departments: {
      $collect: {
        label: 'DEPARTMENT',
        select: { deptName: '$self.name' },
        limit: 10
      }
    }
  }
}

const r43 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(sel0:__RUSHDB__LABEL__RECORD__:\`DEPARTMENT\`)
WITH record, apoc.coll.sortMaps(collect(DISTINCT sel0 {deptName: sel0.\`name\`, __RUSHDB__KEY__LABEL__: [label IN labels(sel0) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "__RUSHDB__KEY__ID__")[0..10] AS \`departments\`
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`name\`: record.\`name\`, \`departments\`} as records`

const q44 = {
  labels: ['COMPANY'],
  select: {
    employees: {
      $collect: {
        label: 'EMPLOYEE',
        where: { salary: { $gte: 50000 } },
        select: { name: '$self.name', salary: '$self.salary' },
        orderBy: { salary: 'desc' },
        limit: 5
      }
    }
  }
}

const r44 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(sel0:__RUSHDB__LABEL__RECORD__:\`EMPLOYEE\`) WHERE ((any(value IN sel0.\`salary\` WHERE value >= 50000)))
WITH record, apoc.coll.sortMaps(collect(DISTINCT sel0 {name: sel0.\`name\`, salary: sel0.\`salary\`, __RUSHDB__KEY__LABEL__: [label IN labels(sel0) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "salary")[0..5] AS \`employees\`
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`employees\`} as records`

const q45 = {
  labels: ['COMPANY'],
  select: {
    departments: {
      $collect: {
        label: 'DEPARTMENT',
        select: {
          name: '$self.name',
          projects: {
            $collect: { label: 'PROJECT', select: { name: '$self.name' }, limit: 10 }
          }
        }
      }
    }
  }
}

const r45 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(sel0:__RUSHDB__LABEL__RECORD__:\`DEPARTMENT\`)
OPTIONAL MATCH (sel0)--(sel1:__RUSHDB__LABEL__RECORD__:\`PROJECT\`)
WITH record, sel0, apoc.coll.sortMaps(collect(DISTINCT sel1 {name: sel1.\`name\`, __RUSHDB__KEY__LABEL__: [label IN labels(sel1) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "__RUSHDB__KEY__ID__")[0..10] AS \`projects\`
WITH record, apoc.coll.sortMaps(collect(DISTINCT sel0 {name: sel0.\`name\`, \`projects\`, __RUSHDB__KEY__LABEL__: [label IN labels(sel0) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "__RUSHDB__KEY__ID__")[0..100] AS \`departments\`
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`departments\`} as records`

const q46 = {
  labels: ['COMPANY'],
  where: { foundedAt: { $lte: { $year: 1980 } } },
  select: {
    departments: {
      $collect: {
        label: 'DEPARTMENT',
        select: {
          projects: {
            $collect: {
              label: 'PROJECT',
              orderBy: { projectName: 'asc' },
              select: {
                employees: {
                  $collect: { label: 'EMPLOYEE', orderBy: { salary: 'desc' }, limit: 3 }
                }
              }
            }
          }
        }
      }
    }
  }
}

const r46 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE (any(value IN record.\`foundedAt\` WHERE apoc.convert.fromJsonMap(record.\`__RUSHDB__KEY__PROPERTIES__META__\`).\`foundedAt\` = "datetime" AND datetime(value) <= datetime({year: 1980}))) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(sel0:__RUSHDB__LABEL__RECORD__:\`DEPARTMENT\`)
OPTIONAL MATCH (sel0)--(sel1:__RUSHDB__LABEL__RECORD__:\`PROJECT\`)
OPTIONAL MATCH (sel1)--(sel2:__RUSHDB__LABEL__RECORD__:\`EMPLOYEE\`)
WITH record, sel0, sel1, apoc.coll.sortMaps(collect(DISTINCT sel2 {.*, __RUSHDB__KEY__LABEL__: [label IN labels(sel2) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "salary")[0..3] AS \`employees\`
WITH record, sel0, apoc.coll.sortMaps(collect(DISTINCT sel1 {\`employees\`, __RUSHDB__KEY__LABEL__: [label IN labels(sel1) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "^projectName")[0..100] AS \`projects\`
WITH record, apoc.coll.sortMaps(collect(DISTINCT sel0 {\`projects\`, __RUSHDB__KEY__LABEL__: [label IN labels(sel0) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "__RUSHDB__KEY__ID__")[0..100] AS \`departments\`
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`departments\`} as records`

const q47 = {
  labels: ['COMPANY'],
  select: {
    departments: {
      $collect: {
        label: 'DEPARTMENT',
        select: {
          name: '$self.name',
          projects: {
            $collect: {
              label: 'PROJECT',
              select: {
                name: '$self.name',
                employees: {
                  $collect: {
                    label: 'EMPLOYEE',
                    orderBy: { salary: 'desc' },
                    limit: 3
                  }
                }
              }
            }
          }
        }
      }
    }
  }
}

const q50 = {
  labels: ['PROJECT'],
  select: {
    project_name: '$record.name',
    top_paid_employees: {
      $collect: {
        label: 'EMPLOYEE',
        select: {
          employee_name: '$self.name',
          salary: '$self.salary'
        },
        orderBy: {
          salary: 'desc'
        },
        limit: 3
      }
    }
  }
}

const q51 = {
  ...q50,
  groupBy: ['$record.name']
}

const q52 = {
  labels: ['DEPARTMENT'],
  where: {
    PROJECT: {
      EMPLOYEE: {
        $alias: '$employee'
      }
    }
  },
  select: {
    department_name: '$record.name',
    top_paid_employees: {
      $collect: {
        from: '$employee',
        select: {
          employee_name: '$employee.name',
          salary: '$employee.salary'
        },
        orderBy: {
          salary: 'desc'
        },
        limit: 3
      }
    }
  }
}

const r47 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(sel0:__RUSHDB__LABEL__RECORD__:\`DEPARTMENT\`)
OPTIONAL MATCH (sel0)--(sel1:__RUSHDB__LABEL__RECORD__:\`PROJECT\`)
OPTIONAL MATCH (sel1)--(sel2:__RUSHDB__LABEL__RECORD__:\`EMPLOYEE\`)
WITH record, sel0, sel1, apoc.coll.sortMaps(collect(DISTINCT sel2 {.*, __RUSHDB__KEY__LABEL__: [label IN labels(sel2) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "salary")[0..3] AS \`employees\`
WITH record, sel0, apoc.coll.sortMaps(collect(DISTINCT sel1 {name: sel1.\`name\`, \`employees\`, __RUSHDB__KEY__LABEL__: [label IN labels(sel1) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "__RUSHDB__KEY__ID__")[0..100] AS \`projects\`
WITH record, apoc.coll.sortMaps(collect(DISTINCT sel0 {name: sel0.\`name\`, \`projects\`, __RUSHDB__KEY__LABEL__: [label IN labels(sel0) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "__RUSHDB__KEY__ID__")[0..100] AS \`departments\`
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`departments\`} as records`

const r50 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`PROJECT\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(sel0:__RUSHDB__LABEL__RECORD__:\`EMPLOYEE\`)
WITH record, apoc.coll.sortMaps(collect(DISTINCT sel0 {employee_name: sel0.\`name\`, salary: sel0.\`salary\`, __RUSHDB__KEY__LABEL__: [label IN labels(sel0) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "salary")[0..3] AS \`top_paid_employees\`
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`project_name\`: record.\`name\`, \`top_paid_employees\`} as records`

const r52 = `MATCH (record:__RUSHDB__LABEL__RECORD__:\`DEPARTMENT\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)--(record1:__RUSHDB__LABEL__RECORD__:\`PROJECT\`)
OPTIONAL MATCH (record1)--(record2:__RUSHDB__LABEL__RECORD__:\`EMPLOYEE\`)
WITH record, record1, record2 WHERE record IS NOT NULL AND (record1 IS NOT NULL AND record2 IS NOT NULL)
WITH record, apoc.coll.sortMaps(collect(DISTINCT record2 {employee_name: record2.\`name\`, salary: record2.\`salary\`, __RUSHDB__KEY__LABEL__: [label IN labels(record2) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "salary")[0..3] AS \`top_paid_employees\`
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`department_name\`: record.\`name\`, \`top_paid_employees\`} as records`

describe('label-based $collect (inline traversal)', () => {
  let queryService: EntityQueryService

  beforeEach(() => {
    queryService = new EntityQueryService()
  })

  it('43 - 1-level label collect with field projection', () => {
    expect(queryService.findRecords({ searchQuery: q43 })).toEqual(r43)
  })

  it('44 - 1-level label collect with $collect.where filter', () => {
    expect(queryService.findRecords({ searchQuery: q44 })).toEqual(r44)
  })

  it('45 - 2-level nested label collect', () => {
    expect(queryService.findRecords({ searchQuery: q45 })).toEqual(r45)
  })

  it('46 - 3-level nested label collect with root where + per-level orderBy', () => {
    expect(queryService.findRecords({ searchQuery: q46 })).toEqual(r46)
  })

  it('47 - 3-level nested label collect (dashboard aggregateExample4)', () => {
    expect(queryService.findRecords({ searchQuery: q47 })).toEqual(r47)
  })

  it('50 - top related records per root with label collect', () => {
    expect(queryService.findRecords({ searchQuery: q50 })).toEqual(r50)
  })

  it('51 - rejects label collect with root groupBy', () => {
    expect(() => queryService.findRecords({ searchQuery: q51 })).toThrow(
      'Label-based $collect cannot be combined with root groupBy'
    )
  })

  it('52 - top related records through an intermediate path with alias collect', () => {
    expect(queryService.findRecords({ searchQuery: q52 })).toEqual(r52)
  })

  it('53 - rejects alias-only groupBy values before Cypher generation', () => {
    expect(() =>
      queryService.findRecords({
        searchQuery: {
          labels: ['PARENT', 'CHILD'],
          select: {
            count: {
              $count: '*'
            }
          },
          groupBy: ['$record'],
          orderBy: {
            count: 'desc'
          }
        }
      })
    ).toThrow(/groupBy field reference "\$record" must include a property name/)
  })
})

describe('variable-length traversal and cycles (complete queries)', () => {
  let queryService: EntityQueryService

  beforeEach(() => {
    queryService = new EntityQueryService()
  })

  it('54 - findRecords with typed hops traversal and alias collect', () => {
    const result = queryService.findRecords({
      searchQuery: {
        labels: ['COMPANY'],
        where: {
          $or: [{ stage: 'seed' }, { stage: 'roundA' }],
          EMPLOYEE: {
            $alias: '$employee',
            $relation: { type: 'MANAGES', direction: 'out', hops: { max: 3 } },
            salary: { $gte: 500_000 }
          }
        },
        select: {
          employees: {
            $collect: { from: '$employee', orderBy: { salary: 'desc' }, limit: 10 }
          }
        }
      }
    })

    expect(result)
      .toEqual(`MATCH (record:__RUSHDB__LABEL__RECORD__:\`COMPANY\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WHERE ((any(value IN record.\`stage\` WHERE value = "seed")) OR (any(value IN record.\`stage\` WHERE value = "roundA"))) ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
OPTIONAL MATCH (record)-[:MANAGES*1..3]->(record1:__RUSHDB__LABEL__RECORD__:\`EMPLOYEE\`) WHERE (any(value IN record1.\`salary\` WHERE value >= 500000))
WITH record, record1 WHERE record IS NOT NULL AND record1 IS NOT NULL
WITH record, apoc.coll.sortMaps(collect(DISTINCT record1 {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record1) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]}), "salary")[0..10] AS \`employees\`
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0], \`employees\`} as records`)
  })

  it('55 - findRecords with a typed cycle', () => {
    const result = queryService.findRecords({
      searchQuery: {
        labels: ['ACCOUNT'],
        where: {
          $cycle: { type: 'TRANSFERRED_TO', direction: 'out', hops: { max: 6 } }
        }
      }
    })

    expect(result)
      .toEqual(`MATCH (record:__RUSHDB__LABEL__RECORD__:\`ACCOUNT\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
ORDER BY record.\`__RUSHDB__KEY__ID__\` DESC SKIP 0 LIMIT 100
WITH record WHERE record IS NOT NULL AND EXISTS { MATCH (record)-[:TRANSFERRED_TO*2..6]->(record) }
RETURN DISTINCT record {.*, __RUSHDB__KEY__LABEL__: [label IN labels(record) WHERE label <> "__RUSHDB__LABEL__RECORD__"][0]} AS records`)
  })

  it('56 - getRecordsCount with a typed cycle keeps DISTINCT record', () => {
    const result = queryService.getRecordsCount({
      searchQuery: {
        labels: ['ACCOUNT'],
        where: {
          $cycle: { type: 'TRANSFERRED_TO', direction: 'out', hops: { max: 6 } }
        }
      }
    })

    expect(result)
      .toEqual(`MATCH (record:__RUSHDB__LABEL__RECORD__:\`ACCOUNT\` { __RUSHDB__KEY__PROJECT__ID__: $projectId })
WITH record WHERE record IS NOT NULL AND EXISTS { MATCH (record)-[:TRANSFERRED_TO*2..6]->(record) }
RETURN count(DISTINCT record) as total`)
  })
})
