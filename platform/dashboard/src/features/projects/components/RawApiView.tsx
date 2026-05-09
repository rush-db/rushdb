import { useEffect, useMemo } from 'react'

import { useStore } from '@nanostores/react'

import { DialogLoadingOverlay } from '~/elements/Dialog.tsx'
import { Button } from '~/elements/Button.tsx'
import { ClipboardPaste, LightbulbIcon, PlayIcon } from 'lucide-react'
import { atom } from 'nanostores'
import { SelectEntityApi } from '~/features/projects/components/SelectEntityApi.tsx'
import { useSearchQuery } from '~/features/projects/utils.ts'
import { Editor } from '~/elements/Editor.tsx'
import { $editorData, $selectedOperation } from '~/features/projects/stores/raw-api.ts'
import { $recordRawApiEntity } from '~/features/projects/stores/current-project.ts'
import { ApiRecordsModal } from '~/features/records/components/ApiRecordsModal.tsx'
import { IconButton } from '~/elements/IconButton'
import { Menu, MenuItem, MenuTitle } from '~/elements/Menu.tsx'

import { Divider } from '~/elements/Divider.tsx'
import { api } from '~/lib/api'
import { CheckboxField } from '~/elements/Checkbox.tsx'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { useCurrentWorkspacePlan } from '~/features/billing/hooks/useBillingHooks'
import {
  useRawLabelsMutation,
  useRawPropertiesMutation,
  useRawRecordsMutation
} from '~/features/projects/hooks/useRawApiMutations'

const $recordsData = atom<string>('')
const $labelsData = atom<string>('')
const $propertiesData = atom<string>('')
const $showCypherQuery = atom<boolean>(true)
const $cypherQuery = atom<string>('')

const aggregateExample0 = `{
    "labels": [
        "DEPARTMENT"
    ],
    "select": {
        "departmentName": "$record.name",
        "departmentDescription": "$record.description",
        "projects": {
            "$collect": {
                "label": "PROJECT",
                "select": {
                    "name": "$self.name"
                }
            }
        }
    },
    "orderBy": "asc",
    "skip": 0,
    "limit": 1000
}`

const aggregateExample1 = `{
    "labels": [
        "PROJECT"
    ],
    "where": {
        "budget": {
            "$lte": 10000000
        },
        "EMPLOYEE": {
            "$alias": "$employee"
        }
    },
    "select": {
        "projectName": "$record.name",
        "projectBudget": "$record.budget",
        "employeesCount": {
            "$count": "$employee"
        },
        "totalWage": {
            "$sum": "$employee.salary"
        },
        "avgSalary": {
            "$avg": "$employee.salary",
            "$precision": 0
        },
        "minSalary": {
            "$min": "$employee.salary"
        },
        "maxSalary": {
            "$max": "$employee.salary"
        }
    },
    "orderBy": "asc",
    "skip": 0,
    "limit": 1000
}`

const aggregateExample2 = `{
    "labels": [
        "COMPANY"
    ],
    "where": {
        "DEPARTMENT": {
            "PROJECT": {
                "EMPLOYEE": {
                    "dob": {
                        "$lte": {
                            "$year": 1994
                        }
                    },
                    "$alias": "$employee"
                }
            }
        }
    },
    "select": {
        "company": "$record.name",
        "employees": {
            "$collect": {
                "from": "$employee",
                "orderBy": {
                    "salary": "desc"
                },
                "limit": 10
            }
        }
    }
}`

const aggregateExample3 = `{
    "labels": [
        "PROJECT"
    ],
    "where": {
        "EMPLOYEE": {
            "$alias": "$employee"
        }
    },
    "select": {
        "projectName": "$record.name",
        "totalSalary": {
            "$sum": "$employee.salary"
        },
        "employeeCount": {
            "$count": "$employee"
        },
        "avgSalary": {
            "$divide": [
                { "$ref": "totalSalary" },
                { "$ref": "employeeCount" }
            ]
        }
    },
    "limit": 10
}`

const aggregateExample4 = `{
    "labels": [
        "COMPANY"
    ],
    "select": {
        "departments": {
            "$collect": {
                "label": "DEPARTMENT",
                "select": {
                    "name": "$self.name",
                    "projects": {
                        "$collect": {
                            "label": "PROJECT",
                            "select": {
                                "name": "$self.name",
                                "employees": {
                                    "$collect": {
                                        "label": "EMPLOYEE",
                                        "orderBy": { "salary": "desc" },
                                        "limit": 3
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}`

const queryExample1 = `{
    "labels": [
        "EMPLOYEE"
    ],
    "where": {
        "name": {
           "$startsWith": "Sam"
        },
        "salary": {
            "$gte": 300000
        }
    },
    "orderBy": "asc",
    "skip": 0,
    "limit": 1000
}`

const queryExample2 = `{
    "labels": [
        "DEPARTMENT"
    ],
    "where": {
        "name": {
            "$contains": "e"
        },
        "PROJECT": {
            "budget": {
                "$xor": {
                    "$lte": 10000000,
                    "$gte": 15000000
                }
            },
            "EMPLOYEE": {
                "salary": {
                    "$gte": 300000
                }
            }
        }
    },
    "orderBy": {
        "name": "asc"
    },
    "skip": 0,
    "limit": 1000
}`

const queryExample3 = `{
  "labels": ["PROJECT"],
  "select": {
    "count": { "$count": "*" }
  },
  "groupBy": ["$record.active"],
  "orderBy": {
    "count": "desc"
  },
  "skip": 0,
  "limit": 1000
}`

const ExampleSelector = () => {
  return (
    <Menu
      trigger={
        <IconButton aria-label="examples" title="Examples" variant="secondary" size="small">
          <LightbulbIcon />
        </IconButton>
      }
      align="end"
    >
      <MenuTitle className="mb-2">Query Examples</MenuTitle>
      <Divider />
      <MenuItem className="h-[64px]" icon={<ClipboardPaste />} onClick={() => $editorData.set(queryExample1)}>
        <div className="text-left">
          Basic Query
          <p className="text-content3 text-xs">Find highly paid employees using simple criteria</p>
        </div>
      </MenuItem>
      <Divider />
      <MenuItem className="h-[64px]" icon={<ClipboardPaste />} onClick={() => $editorData.set(queryExample2)}>
        <div className="text-left">
          Advanced Query
          <p className="text-content3 text-xs">Use complex criteria with logical grouping</p>
        </div>
      </MenuItem>
      <Divider />
      <MenuItem className="h-[64px]" icon={<ClipboardPaste />} onClick={() => $editorData.set(queryExample3)}>
        <div className="text-left">
          GroupBy Query
          <p className="text-content3 text-xs">
            Pivot results by one or more properties (groupBy + aggregations)
          </p>
        </div>
      </MenuItem>
      <Divider />
      <MenuTitle className="mb-2">Select Examples</MenuTitle>
      <Divider />
      <MenuItem
        className="h-[64px]"
        icon={<ClipboardPaste />}
        onClick={() => $editorData.set(aggregateExample0)}
      >
        <div className="text-left">
          Collect with Projection
          <p className="text-content3 text-xs">Retrieve related record fields using $collect</p>
        </div>
      </MenuItem>
      <Divider />
      <MenuItem
        className="h-[64px]"
        icon={<ClipboardPaste />}
        onClick={() => $editorData.set(aggregateExample1)}
      >
        <div className="text-left">
          Aggregation Metrics
          <p className="text-content3 text-xs">Analyze salary distributions using $sum, $avg, $min, $max</p>
        </div>
      </MenuItem>
      <Divider />
      <MenuItem
        className="h-[64px]"
        icon={<ClipboardPaste />}
        onClick={() => $editorData.set(aggregateExample2)}
      >
        <div className="text-left">
          Deep Collect
          <p className="text-content3 text-xs">Traverse relations and collect top employees by salary</p>
        </div>
      </MenuItem>
      <Divider />
      <MenuItem
        className="h-[64px]"
        icon={<ClipboardPaste />}
        onClick={() => $editorData.set(aggregateExample3)}
      >
        <div className="text-left">
          Derived Metrics
          <p className="text-content3 text-xs">Compute a ratio from two aggregations using $ref</p>
        </div>
      </MenuItem>
      <Divider />
      <MenuItem
        className="h-[64px]"
        icon={<ClipboardPaste />}
        onClick={() => $editorData.set(aggregateExample4)}
      >
        <div className="text-left">
          Nested Collect
          <p className="text-content3 text-xs">
            Traverse 3 levels deep and collect records using $collect.label
          </p>
        </div>
      </MenuItem>
    </Menu>
  )
}

export function RawApiView() {
  const query = useStore($editorData)
  const entity = useStore($recordRawApiEntity)
  const { data: platformSettings } = usePlatformSettings()
  const { currentPlan } = useCurrentWorkspacePlan()
  const paidUser = currentPlan && currentPlan.id !== 'free' && currentPlan.id !== 'start'

  const { mutateAsync: findRecords, isPending: recordsSubmitting } = useRawRecordsMutation()
  const { mutateAsync: findLabels, isPending: labelsSubmitting } = useRawLabelsMutation()
  const { mutateAsync: findProperties, isPending: propertiesSubmitting } = useRawPropertiesMutation()

  const recordsData = useStore($recordsData)
  const labelsData = useStore($labelsData)
  const propertiesData = useStore($propertiesData)

  const editorData = useStore($editorData)
  const q = useSearchQuery()
  const operation = useStore($selectedOperation)
  const showCypherQuery = useStore($showCypherQuery)
  const cypherQuery = useStore($cypherQuery)

  useEffect(() => {
    if (operation === 'records.find') {
      $editorData.set(
        JSON.stringify({
          where: {},
          orderBy: {},
          skip: 0,
          limit: 100,
          labels: []
        })
      )
    } else {
      $editorData.set('{}')
    }
  }, [operation])

  useEffect(() => {
    // Inherit current searchQuery to local query on mount (once)
    $editorData.set(JSON.stringify(q))
  }, [])

  const handleSearch = () => {
    $recordsData.set('')
    $labelsData.set('')
    $propertiesData.set('')
    $cypherQuery.set('')

    // Get the search query
    const searchQueryObj = JSON.parse(editorData ?? '{}')

    // Fetch the Cypher query if we're doing records.find and the toggle is enabled
    if (operation === 'records.find' && showCypherQuery) {
      api.query['records-find']({
        searchQuery: searchQueryObj
      })
        .then((cypherQueryStr) => {
          $cypherQuery.set(cypherQueryStr)
        })
        .catch((error) => {
          console.error('Failed to fetch Cypher query:', error)
        })
    }

    findRecords({
      searchQuery: searchQueryObj
    }).then((response) => {
      const { data, total } = response as unknown as {
        data?: Array<{ data?: unknown }> | Record<string, number>
        total?: number
      }
      $recordsData.set(
        JSON.stringify({
          data: Array.isArray(data) ? data.map((record) => record.data ?? record) : data,
          total
        })
      )
    })

    findLabels({
      searchQuery: searchQueryObj
    }).then((response) => {
      const { data, total } = response as unknown as { data?: unknown; total?: number }
      $labelsData.set(JSON.stringify({ data, total }))
    })

    findProperties({
      searchQuery: searchQueryObj
    }).then((response) => {
      const { data, total } = response as unknown as { data?: unknown; total?: number }
      $propertiesData.set(JSON.stringify({ data, total }))
    })
  }

  const result = useMemo(() => {
    if (entity === 'records') {
      return recordsData
    }

    if (entity === 'labels') {
      return labelsData
    }

    if (entity === 'properties') {
      return propertiesData
    }
  }, [entity, recordsData, labelsData, propertiesData])

  return (
    <>
      <div className="grid h-full grid-cols-2 grid-rows-2 gap-5 px-5 pb-5">
        <div className="row-span-2 flex h-full min-h-[80vh] flex-col">
          <div className="border-r pr-5">
            <div className="my-5 flex w-full items-center justify-between">
              <div className="flex w-full items-end justify-between gap-3">
                <p className="text-content2 mb-2 text-lg">Payload</p>
                <div className="flex w-full items-center justify-end gap-3">
                  {/*  <p className="text-content2 mb-2 text-lg">Method</p>*/}
                  {/*  <OperationSelector />*/}

                  {operation === 'records.find' && (
                    <CheckboxField
                      className="mb-0 mr-2"
                      label="Show Cypher"
                      checked={showCypherQuery}
                      onCheckedChange={$showCypherQuery.set}
                    />
                  )}

                  <ApiRecordsModal />

                  {['records.find', 'records.findUniq', 'records.findOne'].includes(operation) && (
                    <ExampleSelector />
                  )}

                  <Button
                    onClick={handleSearch}
                    loading={recordsSubmitting || labelsSubmitting || propertiesSubmitting}
                    size="small"
                    className="min-w-0 px-3"
                    variant="secondary"
                  >
                    <PlayIcon className="h-2 w-2" />
                  </Button>
                </div>
              </div>
            </div>

            <Editor
              defaultLanguage="json"
              value={query}
              onChange={$editorData.set}
              height={operation === 'records.find' && showCypherQuery && cypherQuery ? '50vh' : '80vh'}
              format={false}
              theme="vs-dark"
            />

            {operation === 'records.find' && showCypherQuery && cypherQuery && (
              <div className="mt-4">
                <p className="text-content2 mb-2 text-lg">Generated Cypher Query</p>
                <Editor defaultLanguage="cypher" value={cypherQuery} height="25vh" readOnly theme="vs-dark" />
              </div>
            )}
          </div>
        </div>

        <div className="col-start-2 row-span-2 row-start-1 flex flex-col">
          <>
            <div className="my-5 flex w-full items-center justify-between">
              <p className="text-content2 text-lg">Result</p>
              <SelectEntityApi />
            </div>
            <Editor defaultLanguage="json" value={result} height="100%" readOnly lineNumbers="off" />
          </>
        </div>
      </div>

      {(recordsSubmitting || labelsSubmitting || propertiesSubmitting) && <DialogLoadingOverlay />}
    </>
  )
}
