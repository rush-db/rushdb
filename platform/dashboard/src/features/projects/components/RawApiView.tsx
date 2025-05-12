import React, { useEffect, useMemo } from 'react'

import { useStore } from '@nanostores/react'

import { DialogLoadingOverlay } from '~/elements/Dialog.tsx'
import { Button } from '~/elements/Button.tsx'
import { ClipboardPaste, LightbulbIcon, PlayIcon } from 'lucide-react'
import { atom } from 'nanostores'
import { SelectEntityApi } from '~/features/projects/components/SelectEntityApi.tsx'
import { useSearchQuery } from '~/features/projects/utils.ts'
import { Editor } from '~/elements/Editor.tsx'
import {
  $editorData,
  $selectedOperation,
  rawLabels,
  rawProperties,
  rawRecords
} from '~/features/projects/stores/raw-api.ts'
import { DBRecordsArrayInstance } from '@rushdb/javascript-sdk'
import { $recordRawApiEntity } from '~/features/projects/stores/current-project.ts'
import { ApiRecordsModal } from '~/features/records/components/ApiRecordsModal.tsx'
import { IconButton } from '~/elements/IconButton'
import { Menu, MenuItem, MenuTitle } from '~/elements/Menu.tsx'

import { Divider } from '~/elements/Divider.tsx'
import { Select } from '~/elements/Select.tsx'

const $recordsData = atom<string>('')
const $labelsData = atom<string>('')
const $propertiesData = atom<string>('')

const aggregateExample0 = `{
    "labels":[
        "DEPARTMENT"
    ],
    "where": {
        "PROJECT": {
            "$alias": "$project"
        }
    },
    "aggregate": {
        "departmentName": "$record.name",
        "departmentDescription": "$record.description",
        "projects": {
            "fn": "collect",
            "uniq": true,
            "field": "name",
            "alias": "$project"
        }
    },
    "orderBy": "asc",
    "skip": 0,
    "limit": 1000
}`

const aggregateExample1 = `{
    "labels":[
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
    "aggregate": {
        "projectName": "$record.name",
        "projectBudget": "$record.budget",
        "employeesCount": {
            "fn": "count",
            "uniq": true,
            "alias": "$employee"
        },
        "totalWage": {
            "fn": "sum",
            "field": "salary",
            "alias": "$employee"
        },
        "avgSalary": {
            "fn": "avg",
            "field": "salary",
            "alias": "$employee",
            "precision": 0
        },
        "minSalary": {
            "fn": "min",
            "field": "salary",
            "alias": "$employee"
        },
        "maxSalary": {
            "fn": "max",
            "field": "salary",
            "alias": "$employee"
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
    "aggregate": {
        "company": "$record.name",
        "employees": {
            "fn": "collect",
            "alias": "$employee",
            "orderBy": {
                "salary": "desc"
            },
            "limit": 10
        }
    }
}`

const aggregateExample3 = `{
    "labels": [
        "COMPANY"
    ],
    "where": {
        "foundedAt": {
            "$lte": {
                "$year": 1980
            }
        },
        "DEPARTMENT": {
            "$alias": "$department",
            "PROJECT": {
                "$alias": "$project",
                "EMPLOYEE": {
                    "$alias": "$employee"
                }
            }
        }
    },
    "aggregate": {
        "departments": {
            "fn": "collect",
            "alias": "$department",
            "aggregate": {
                "projects": {
                    "fn": "collect",
                    "alias": "$project",
                    "orderBy": {
                        "projectName": "asc",
                        "projectId": "desc"
                    },
                    "aggregate": {
                        "employees": {
                            "fn": "collect",
                            "orderBy": {
                                "salary": "desc"
                            },
                            "alias": "$employee",
                            "limit": 3
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
      <MenuTitle className="mb-2">Aggregation Examples</MenuTitle>
      <Divider />
      <MenuItem
        className="h-[64px]"
        icon={<ClipboardPaste />}
        onClick={() => $editorData.set(aggregateExample0)}
      >
        <div className="text-left">
          Basic Aggregation
          <p className="text-content3 text-xs">Retrieve a list of project names</p>
        </div>
      </MenuItem>
      <Divider />
      <MenuItem
        className="h-[64px]"
        icon={<ClipboardPaste />}
        onClick={() => $editorData.set(aggregateExample1)}
      >
        <div className="text-left">
          Advanced Aggregation
          <p className="text-content3 text-xs">Analyze salary distributions and budget data</p>
        </div>
      </MenuItem>
      <Divider />
      <MenuItem
        className="h-[64px]"
        icon={<ClipboardPaste />}
        onClick={() => $editorData.set(aggregateExample2)}
      >
        <div className="text-left">
          Deep Aggregation
          <p className="text-content3 text-xs">Perform deep traversal to retrieve employees</p>
        </div>
      </MenuItem>
      <Divider />
      <MenuItem
        className="h-[64px]"
        icon={<ClipboardPaste />}
        onClick={() => $editorData.set(aggregateExample3)}
      >
        <div className="text-left">
          Nested Aggregation
          <p className="text-content3 text-xs">Traverse and return the entire graph topology</p>
        </div>
      </MenuItem>
    </Menu>
  )
}

const OperationSelector = () => {
  const operation = useStore($selectedOperation)

  const options: Array<{ value: ReturnType<typeof $selectedOperation.get>; label: string }> = [
    { value: 'records.find', label: 'records.find' },
    { value: 'records.findOne', label: 'records.findOne' },
    { value: 'records.findById', label: 'records.findById' },
    { value: 'records.findUniq', label: 'records.findUniq' },
    { value: 'records.deleteById', label: 'records.deleteById' },
    { value: 'records.delete', label: 'records.delete' },
    { value: 'records.createMany', label: 'records.createMany' },
    { value: 'records.export', label: 'records.export' },
    { value: 'records.set', label: 'records.set' },
    { value: 'records.update', label: 'records.update' },
    { value: 'records.attach', label: 'records.attach' },
    { value: 'records.detach', label: 'records.detach' },
    { value: 'labels.find', label: 'labels.find' },
    { value: 'properties.values', label: 'properties.values' },
    { value: 'properties.find', label: 'properties.list' },
    { value: 'relations.find', label: 'relations.find' }
  ]

  return (
    <Select
      onChange={(value: { target: { value: ReturnType<typeof $selectedOperation.get> } }) => {
        $selectedOperation.set(value.target.value)
      }}
      size="small"
      value={operation}
      options={options}
    />
  )
}

export function RawApiView() {
  const query = useStore($editorData)
  const entity = useStore($recordRawApiEntity)

  const { mutate: findRecords, loading: recordsSubmitting } = useStore(rawRecords)
  const { mutate: findLabels, loading: labelsSubmitting } = useStore(rawLabels)
  const { mutate: findProperties, loading: propertiesSubmitting } = useStore(rawProperties)

  const recordsData = useStore($recordsData)
  const labelsData = useStore($labelsData)
  const propertiesData = useStore($propertiesData)

  const editorData = useStore($editorData)

  const q = useSearchQuery()

  useEffect(() => {
    // Inherit current searchQuery to local query on mount (once)
    $editorData.set(JSON.stringify(q))
  }, [])

  const handleSearch = () => {
    $recordsData.set('')
    $labelsData.set('')
    $propertiesData.set('')

    findRecords({
      searchQuery: JSON.parse(editorData ?? '{}')
    }).then((response) => {
      const { data, total } = response as DBRecordsArrayInstance<any>
      $recordsData.set(JSON.stringify({ data: data?.map?.((d) => d.data) ?? data, total }))
    })

    findLabels({
      searchQuery: JSON.parse(editorData ?? '{}')
    }).then((response) => {
      const { data, total } = response as DBRecordsArrayInstance<any>
      $labelsData.set(JSON.stringify({ data, total }))
    })

    findProperties({
      searchQuery: JSON.parse(editorData ?? '{}')
    }).then((response) => {
      const { data, total } = response as DBRecordsArrayInstance<any>
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
              <div className="flex w-full items-end gap-3">
                <div className="flex w-full flex-col">
                  <p className="text-content2 mb-2 text-lg">Method</p>
                  <OperationSelector />
                </div>
                <ApiRecordsModal />

                <ExampleSelector />

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
            <p className="text-content2 mb-2 text-lg">Payload</p>

            <Editor
              defaultLanguage="json"
              value={query}
              onChange={$editorData.set}
              height="80vh"
              format={false}
              theme="vs-dark"
            />
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
