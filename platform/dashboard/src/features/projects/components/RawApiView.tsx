import { type FormEvent, useEffect, useMemo, useRef, useState } from 'react'

import { useStore } from '@nanostores/react'
import { useMutation } from '@tanstack/react-query'

import { Dialog, DialogFooter, DialogLoadingOverlay, DialogTitle } from '~/elements/Dialog.tsx'
import { Button } from '~/elements/Button.tsx'
import { Bookmark, ClipboardPaste, LightbulbIcon, LoaderCircle, PlayIcon, Sparkles } from 'lucide-react'
import { atom } from 'nanostores'
import { SelectEntityApi } from '~/features/projects/components/SelectEntityApi.tsx'
import { useSearchQuery } from '~/features/projects/utils.ts'
import { Editor } from '~/elements/Editor.tsx'
import { Input, TextField } from '~/elements/Input.tsx'
import { Tooltip } from '~/elements/Tooltip.tsx'
import { toast } from '~/elements/Toast.tsx'
import {
  $editorData,
  $selectedOperation,
  onboardingAgentRunSelectQuery
} from '~/features/projects/stores/raw-api.ts'
import { $recordRawApiEntity } from '~/features/projects/stores/current-project.ts'
import { $currentProjectId } from '~/features/projects/stores/id'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { useSaveQueryMutation } from '~/features/saved-queries/hooks'
import { IconButton } from '~/elements/IconButton'
import { Menu, MenuItem, MenuTitle } from '~/elements/Menu.tsx'

import { Divider } from '~/elements/Divider.tsx'
import { api } from '~/lib/api'
import { CheckboxField } from '~/elements/Checkbox.tsx'
import {
  useRawLabelsMutation,
  useRawPropertiesMutation,
  useRawRecordsMutation
} from '~/features/projects/hooks/useRawApiMutations'
import { $tourStep, setTourStep } from '~/features/tour/stores/tour'

const $recordsData = atom<string>('')
const $labelsData = atom<string>('')
const $propertiesData = atom<string>('')
const $showCypherQuery = atom<boolean>(true)
const $cypherQuery = atom<string>('')

function parseEditorPayload(value: string | undefined): Record<string, unknown> {
  try {
    const parsed = JSON.parse(value || '{}')
    return parsed && typeof parsed === 'object' ? (parsed as Record<string, unknown>) : {}
  } catch {
    return {}
  }
}

const rawApiOnboardingSteps = new Set(['rawApiSelectQuery', 'rawApiRunQuery', 'rawApiResults'])

const aggregateExample0 = `{
    "labels": [
        "FACTION"
    ],
    "select": {
        "factionName": "$record.name",
        "militaryStrength": "$record.military_strength_score",
        "starships": {
            "$collect": {
                "label": "STARSHIP",
                "select": {
                    "name": "$self.name",
                    "isMilitary": "$self.is_military",
                    "firepower": "$self.firepower_score"
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
        "FACTION"
    ],
    "where": {
        "STARSHIP": {
            "$alias": "$starship",
            "is_military": true
        }
    },
    "select": {
        "factionName": "$record.name",
        "militaryShipCount": {
            "$count": "$starship"
        },
        "totalCrewCapacity": {
            "$sum": "$starship.crew_capacity"
        },
        "avgFirepower": {
            "$avg": "$starship.firepower_score",
            "$precision": 0
        },
        "minCrewCapacity": {
            "$min": "$starship.crew_capacity"
        },
        "maxCrewCapacity": {
            "$max": "$starship.crew_capacity"
        }
    },
    "groupBy": ["$record.name"],
    "orderBy": {
        "militaryShipCount": "desc"
    }
}`

const aggregateExample2 = `{
    "labels": [
        "PLANET"
    ],
    "where": {
        "BATTLE": {
            "STARSHIP": {
                "$alias": "$starship",
                "firepower_score": {
                    "$gte": 60
                }
            }
        }
    },
    "select": {
        "planetName": "$record.name",
        "highFirepowerStarships": {
            "$collect": {
                "from": "$starship",
                "select": {
                    "name": "$starship.name",
                    "firepower": "$starship.firepower_score",
                    "crewCapacity": "$starship.crew_capacity"
                },
                "orderBy": {
                    "firepower": "desc"
                },
                "limit": 10
            }
        }
    }
}`

const aggregateExample3 = `{
    "labels": [
        "STARSHIP"
    ],
    "where": {
        "is_military": true
    },
    "select": {
        "totalCrewCapacity": {
            "$sum": "$record.crew_capacity"
        },
        "shipCount": {
            "$count": "*"
        },
        "crewPerShip": {
            "$divide": [
                { "$ref": "totalCrewCapacity" },
                { "$ref": "shipCount" }
            ]
        }
    },
    "groupBy": [
        "totalCrewCapacity",
        "shipCount",
        "crewPerShip"
    ],
    "orderBy": {
        "crewPerShip": "desc"
    }
}`

const aggregateExample4 = `{
    "labels": [
        "PLANET"
    ],
    "select": {
        "battles": {
            "$collect": {
                "label": "BATTLE",
                "select": {
                    "name": "$self.name",
                    "starships": {
                        "$collect": {
                            "label": "STARSHIP",
                            "select": {
                                "name": "$self.name",
                                "firepower": "$self.firepower_score"
                            },
                            "orderBy": { "firepower": "desc" },
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
        "STARSHIP"
    ],
    "where": {
        "name": {
           "$contains": "Star"
        },
        "is_military": true,
        "firepower_score": {
            "$gte": 80
        }
    },
    "orderBy": {
        "firepower_score": "desc"
    },
    "skip": 0,
    "limit": 1000
}`

const queryExample2 = `{
    "labels": [
        "BATTLE"
    ],
    "where": {
        "name": {
            "$contains": "Battle"
        },
        "$and": [
            {
                "is_space_battle": true
            },
            {
                "$or": [
                    {
                        "estimated_military_casualties": {
                            "$gte": 500000
                        }
                    },
                    {
                        "destruction_level_score": {
                            "$gte": 90
                        }
                    }
                ]
            }
        ]
    },
    "orderBy": {
        "started_at": "asc"
    },
    "skip": 0,
    "limit": 1000
}`

const queryExample3 = `{
  "labels": ["BATTLE"],
  "select": {
    "count": { "$count": "*" },
    "avgMilitaryCasualties": {
      "$avg": "$record.estimated_military_casualties",
      "$precision": 0
    }
  },
  "groupBy": ["$record.era"],
  "orderBy": {
    "count": "desc"
  }
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
      <MenuTitle className="mb-2 text-lg">Query Examples</MenuTitle>
      <p className="px-4 pb-2 text-sm leading-4 text-content3">
        These examples use the sample dataset available on the Import Data page.
      </p>
      <Divider />
      <MenuItem className="h-[64px]" icon={<ClipboardPaste />} onClick={() => $editorData.set(queryExample1)}>
        <div className="text-left">
          Basic Query
          <p className="text-xs text-content3">Find highly paid employees using simple criteria</p>
        </div>
      </MenuItem>
      <Divider />
      <MenuItem className="h-[64px]" icon={<ClipboardPaste />} onClick={() => $editorData.set(queryExample2)}>
        <div className="text-left">
          Advanced Query
          <p className="text-xs text-content3">Use complex criteria with logical grouping</p>
        </div>
      </MenuItem>
      <Divider />
      <MenuItem className="h-[64px]" icon={<ClipboardPaste />} onClick={() => $editorData.set(queryExample3)}>
        <div className="text-left">
          GroupBy Query
          <p className="text-xs text-content3">
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
          <p className="text-xs text-content3">Retrieve related record fields using $collect</p>
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
          <p className="text-xs text-content3">Analyze salary distributions using $sum, $avg, $min, $max</p>
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
          <p className="text-xs text-content3">Traverse relations and collect top employees by salary</p>
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
          <p className="text-xs text-content3">Compute a ratio from two aggregations using $ref</p>
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
          <p className="text-xs text-content3">
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
  const tourStep = useStore($tourStep)
  const hasSeededOnboardingQuery = useRef(false)

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

  const projectId = useStore($currentProjectId)
  const { data: platformSettings, isPending: loadingSettings } = usePlatformSettings()
  const llmEnabled = platformSettings?.llmEnabled === true

  // AI mode: a natural-language prompt that generates a SearchQuery into the payload editor
  // (same backend flow as Smart Search). The last generated prompt is captured so it can be
  // persisted when the query is saved.
  const [aiPrompt, setAiPrompt] = useState('')
  const [generatedFromPrompt, setGeneratedFromPrompt] = useState<string | undefined>()

  const generateSearchQuery = useMutation({
    mutationFn: (prompt: string) =>
      api.ai.generateSearchQuery({
        projectId: projectId!,
        prompt,
        currentQuery: parseEditorPayload(editorData)
      }),
    onSuccess(result, prompt) {
      $editorData.set(JSON.stringify(result.searchQuery, null, 2))
      setGeneratedFromPrompt(prompt)
      if (result.warnings?.length) {
        toast({ title: 'Query adjusted', description: result.warnings.join(' ') })
      }
    },
    onError(error: unknown) {
      toast({
        title: 'AI search failed',
        description: error instanceof Error ? error.message : 'Could not generate a query',
        variant: 'danger'
      })
    }
  })

  const aiDisabled = loadingSettings || !llmEnabled || generateSearchQuery.isPending

  const submitAiPrompt = (event: FormEvent) => {
    event.preventDefault()
    if (aiDisabled || !aiPrompt.trim()) {
      return
    }
    generateSearchQuery.mutate(aiPrompt.trim())
  }

  // Editing the payload by hand invalidates the captured AI prompt.
  const onEditorChange = (value?: string) => {
    setGeneratedFromPrompt(undefined)
    $editorData.set(value ?? '')
  }

  const { mutateAsync: saveQuery, isPending: saving } = useSaveQueryMutation()
  const [saveOpen, setSaveOpen] = useState(false)
  const [saveName, setSaveName] = useState('')

  const openSaveDialog = () => {
    setSaveName(`My Query ${new Date().toISOString()}`)
    setSaveOpen(true)
  }

  const handleSave = async () => {
    const searchQuery = parseEditorPayload(editorData)
    try {
      await saveQuery({
        name: saveName.trim(),
        searchMode: generatedFromPrompt ? 'ai' : 'manual',
        searchQuery,
        prompt: generatedFromPrompt
      })
      toast({ title: 'Query saved', description: `"${saveName.trim()}" is now in Saved Queries.` })
      setSaveOpen(false)
    } catch (error) {
      toast({
        title: 'Failed to save query',
        description: error instanceof Error ? error.message : 'Please try again',
        variant: 'danger'
      })
    }
  }

  useEffect(() => {
    // Don't reset editor during onboarding — seeding effect takes precedence
    if (rawApiOnboardingSteps.has($tourStep.get())) return
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
    if (rawApiOnboardingSteps.has($tourStep.get())) {
      $editorData.set(onboardingAgentRunSelectQuery)
      hasSeededOnboardingQuery.current = true
      return
    }

    // Inherit current searchQuery to local query on mount (once)
    $editorData.set(JSON.stringify(q))
  }, [])

  useEffect(() => {
    if (!rawApiOnboardingSteps.has(tourStep)) {
      hasSeededOnboardingQuery.current = false
      return
    }

    $recordRawApiEntity.set('records')

    if (operation !== 'records.find') {
      $selectedOperation.set('records.find')
      return
    }

    if (!hasSeededOnboardingQuery.current) {
      $editorData.set(onboardingAgentRunSelectQuery)
      hasSeededOnboardingQuery.current = true
    }
  }, [operation, tourStep])

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

      if (tourStep === 'rawApiRunQuery') {
        setTourStep('rawApiResults', true)
      }
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
            <div className="mb-4 flex w-full items-center justify-between">
              <div className="flex w-full items-end justify-between gap-3">
                <p className="mb-2 text-lg text-content2">Payload</p>
                <div className="flex w-full items-center justify-end gap-3">
                  {/*  <p className="text-content2 mb-2 text-lg">Method</p>*/}
                  {/*  <OperationSelector />*/}

                  {operation === 'records.find' && (
                    <CheckboxField
                      className="mr-2 mb-0"
                      label="Show Cypher"
                      checked={showCypherQuery}
                      onCheckedChange={$showCypherQuery.set}
                    />
                  )}

                  {['records.find', 'records.findUniq', 'records.findOne'].includes(operation) && (
                    <ExampleSelector />
                  )}

                  <Button onClick={openSaveDialog} size="small" variant="outline">
                    <Bookmark />
                    Save query
                  </Button>

                  <Button
                    data-tour="raw-api-run-query"
                    onClick={handleSearch}
                    loading={recordsSubmitting || labelsSubmitting || propertiesSubmitting}
                    size="small"
                    className="min-w-0 px-3"
                    variant={tourStep === 'rawApiRunQuery' ? 'primary' : 'secondary'}
                  >
                    <PlayIcon className="h-2 w-2" />
                  </Button>
                </div>
              </div>
            </div>

            <form className="mb-3 flex min-w-0 items-center gap-3" onSubmit={submitAiPrompt}>
              <Input
                className="w-full"
                disabled={aiDisabled}
                onChange={(event) => setAiPrompt(event.target.value)}
                placeholder={
                  !loadingSettings && !llmEnabled ?
                    'AI query generation is not configured'
                  : 'Describe the query you want to build'
                }
                prefix={<Sparkles className="text-accent" />}
                size="small"
                type="search"
                value={aiPrompt}
              />
              <Tooltip
                trigger={
                  <Button
                    aria-label="generate-ai-query"
                    disabled={aiDisabled || !aiPrompt.trim()}
                    size="small"
                    type="submit"
                    variant="primary"
                  >
                    {generateSearchQuery.isPending ?
                      <LoaderCircle className="animate-spin" />
                    : <Sparkles />}
                    Generate
                  </Button>
                }
              >
                <div className="flex items-center gap-1 text-2xs text-content uppercase">
                  Generate query with AI
                </div>
              </Tooltip>
            </form>

            <div data-tour="raw-api-payload">
              <Editor
                defaultLanguage="json"
                value={query}
                onChange={onEditorChange}
                height={operation === 'records.find' && showCypherQuery && cypherQuery ? '50vh' : '80vh'}
                format={false}
                theme="vs-dark"
              />
            </div>

            {operation === 'records.find' && showCypherQuery && cypherQuery && (
              <div className="mt-4">
                <p className="mb-2 text-lg text-content2">Generated Cypher Query</p>
                <Editor defaultLanguage="cypher" value={cypherQuery} height="25vh" readOnly theme="vs-dark" />
              </div>
            )}
          </div>
        </div>

        <div className="col-start-2 row-span-2 row-start-1 flex flex-col">
          <>
            <div className="mb-4 flex w-full items-center justify-between">
              <p className="text-lg text-content2">Result</p>
              <SelectEntityApi />
            </div>
            <div className="min-h-0 flex-1" data-tour="raw-api-result">
              <Editor defaultLanguage="json" value={result} height="100%" readOnly lineNumbers="off" />
            </div>
          </>
        </div>
      </div>

      {(recordsSubmitting || labelsSubmitting || propertiesSubmitting) && <DialogLoadingOverlay />}

      <Dialog
        className="sm:max-w-md"
        onOpenChange={(next) => {
          setSaveOpen(next)
          if (!next) setSaveName('')
        }}
        open={saveOpen}
      >
        <DialogTitle>Save query</DialogTitle>
        <p className="mb-3 text-sm text-content2">
          Save the current payload to Saved Queries so you can re-run it later
          {generatedFromPrompt ? ', including the AI prompt that generated it' : ''}.
        </p>
        <TextField
          autoFocus
          label="Name"
          onChange={(event) => setSaveName(event.target.value)}
          placeholder="My Query"
          value={saveName}
        />
        <DialogFooter className="mt-4">
          <Button onClick={() => setSaveOpen(false)} variant="outline">
            Cancel
          </Button>
          <Button disabled={!saveName.trim()} loading={saving} onClick={handleSave} variant="primary">
            Save
          </Button>
        </DialogFooter>
      </Dialog>
    </>
  )
}
