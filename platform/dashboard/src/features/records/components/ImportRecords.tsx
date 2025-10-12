import { useStore } from '@nanostores/react'
import { Braces, DatabaseZap, Edit, TestTube2, Table, Settings2 } from 'lucide-react'
import { atom, onSet } from 'nanostores'
import { type ReactNode, Suspense, lazy, useState, ChangeEvent, useEffect } from 'react'

import { Button } from '~/elements/Button'
import { DialogFooter, DialogLoadingOverlay } from '~/elements/Dialog'
import { TextField } from '~/elements/Input'
import { cn } from '~/lib/utils'

import { importJson, importCsv } from '../stores/batch'
import { CheckboxField } from '~/elements/Checkbox.tsx'
import { $router, getRoutePath } from '~/lib/router.ts'
import { $currentProjectId } from '~/features/projects/stores/id.ts'
import { PageHeader, PageTitle } from '~/elements/PageHeader.tsx'
import { setTourStep } from '~/features/tour/stores/tour.ts'

function RadioGroup({ className, ...props }: TInheritableElementProps<'div', {}>) {
  return <div className={cn(className, 'flex flex-col gap-5')} {...props} />
}

function RadioButton({
  title,
  description,
  icon,
  className,
  ...props
}: TInheritableElementProps<
  'button',
  {
    description: ReactNode
    icon: ReactNode
    title: ReactNode
  }
>) {
  return (
    <button
      className={cn(
        'bg-secondary ring-accent-ring hover:border-accent-hover hover:bg-secondary-hover focus-visible:border-accent-focus group-hover:border-accent-hover group-hover:bg-secondary-hover flex w-full items-start gap-5 rounded-lg border px-5 py-4 text-start transition-all focus-visible:ring group-focus:ring',
        className
      )}
      {...props}
    >
      <div className="text-accent mt-2.5 flex items-center justify-center">{icon}</div>
      <div className="flex flex-col">
        <h3 className="font-bold">{title}</h3>
        <p className="text-content2">{description}</p>
      </div>
    </button>
  )
}

const Editor = lazy(() =>
  import('@monaco-editor/react').then((module) => ({
    default: module.Editor
  }))
)

const $step = atom<IngestModalSteps>('method')
const $mode = atom<'json' | 'csv'>('json')
const $csvData = atom<string>('')
const $editorData = atom<string>('')
const $label = atom<string>('')

onSet($editorData, ({ newValue }) => {
  if (newValue.length > 0) {
    $step.set('editor')
  } else {
    $step.set('method')
  }
})

function EditorStep() {
  const [loading, setLoading] = useState(true)
  const defaultValue = useStore($editorData)
  const { mutate, loading: submitting } = useStore(importJson)
  const { mutate: mutateCsv, loading: csvSubmitting } = useStore(importCsv)
  const projectId = useStore($currentProjectId)
  const mode = useStore($mode)
  const csvData = useStore($csvData)

  const [suggestTypes, setSuggestTypes] = useState(true)
  const [castNumberArraysToVectors, setCastNumberArraysToVectors] = useState(false)
  const [convertNumericValuesToNumbers, setConvertNumericValuesToNumbers] = useState(false)
  const [capitalizeLabels, setCapitalizeLabels] = useState(true)
  const [relationshipType, setRelationshipType] = useState('__RUSHDB__RELATION__DEFAULT__')
  const label = useStore($label)

  const [error, setError] = useState<string | undefined>()

  const handleEditorMount = (editor: any) => {
    setTimeout(function () {
      editor
        .getAction('editor.action.formatDocument')
        ?.run()
        .then(() => setLoading(false))
    }, 50)
    editor.getAction('editor.action.formatDocument')?.run()
    editor.onDidBlurEditorWidget(() => {
      editor.getAction('editor.action.formatDocument')?.run()
    })
    editor.onDidFocusEditorWidget(() => {
      editor.getAction('editor.action.formatDocument')?.run()
    })
  }

  // CSV Parse Config local state
  const [delimiter, setDelimiter] = useState(',')
  const [header, setHeader] = useState(true)
  const [skipEmptyLines, setSkipEmptyLines] = useState(true)
  const [quoteChar, setQuoteChar] = useState('"')
  const [escapeChar, setEscapeChar] = useState('"')
  const [newline, setNewline] = useState<string | undefined>(undefined)
  const [dynamicTyping, setDynamicTyping] = useState<boolean | undefined>(undefined)

  // When switching to CSV mode, ensure loading overlay is cleared (since Monaco editor isn't mounted)
  useEffect(() => {
    if (mode === 'csv' && loading) {
      setLoading(false)
    }
    if (mode === 'json') {
      // Reset loading so JSON editor can show skeleton until mounted
      setLoading(true)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode])

  return (
    <>
      <div className="mt-5 flex gap-5">
        <TextField
          caption={
            mode === 'json' ?
              'Specify a Label for the top-level Record(s) parsed from this JSON'
            : 'Specify a Label for the Record(s) created from each CSV row'
          }
          required={true}
          label="Label *"
          onChange={(event: { target: { value: string } }) => {
            setError(undefined)
            $label.set(event.target.value)
          }}
          size="small"
          value={label}
          error={error}
        />
        <TextField
          label="Relationship Type"
          caption="Optionally specify a Type for Relationships to be created between nested Records"
          onChange={(event: { target: { value: string } }) => {
            setRelationshipType(event.target.value)
          }}
          size="small"
          className="flex-auto"
          value={relationshipType}
        />
      </div>
      <div className="flex gap-5">
        <CheckboxField
          className="mb-5 mt-5"
          label="Suggest data types"
          onCheckedChange={() => {
            setSuggestTypes(!suggestTypes)
          }}
          checked={suggestTypes}
        />
        <CheckboxField
          className="mb-5 mt-5"
          label="Capitalize labels"
          onCheckedChange={() => {
            setCapitalizeLabels(!capitalizeLabels)
          }}
          checked={capitalizeLabels}
        />
        <CheckboxField
          className="mb-5 mt-5"
          label="Cast number arrays to vectors"
          onCheckedChange={() => {
            setCastNumberArraysToVectors(!castNumberArraysToVectors)
          }}
          checked={castNumberArraysToVectors}
        />
        <CheckboxField
          className="mb-5 mt-5"
          label="Cast numeric values to numbers"
          onCheckedChange={() => {
            setConvertNumericValuesToNumbers(!convertNumericValuesToNumbers)
          }}
          checked={convertNumericValuesToNumbers}
        />
      </div>

      {mode === 'csv' && (
        <div className="mb-2 rounded border p-4">
          <div className="mb-3 flex items-center gap-2 font-semibold">
            <Settings2 size={16} /> CSV Parse Config
          </div>
          <div className="grid grid-cols-2 gap-3 md:grid-cols-3">
            <TextField
              label="Delimiter"
              size="small"
              value={delimiter}
              onChange={(e: any) => setDelimiter(e.target.value)}
            />
            <TextField
              label="Quote Char"
              size="small"
              value={quoteChar}
              onChange={(e: any) => setQuoteChar(e.target.value)}
            />
            <TextField
              label="Escape Char"
              size="small"
              value={escapeChar}
              onChange={(e: any) => setEscapeChar(e.target.value)}
            />
            <TextField
              label="Newline"
              size="small"
              placeholder="auto"
              value={newline ?? ''}
              onChange={(e: any) => setNewline(e.target.value || undefined)}
            />
            <CheckboxField
              className="mt-2"
              label="Header Row"
              checked={header}
              onCheckedChange={() => setHeader(!header)}
            />
            <CheckboxField
              className="mt-2"
              label="Skip Empty"
              checked={skipEmptyLines}
              onCheckedChange={() => setSkipEmptyLines(!skipEmptyLines)}
            />
            <CheckboxField
              className="mt-2"
              label="Dynamic Typing"
              checked={dynamicTyping ?? false}
              onCheckedChange={() => setDynamicTyping(dynamicTyping ? undefined : true)}
            />
          </div>
          <div className="text-content2 mt-3 text-xs">
            Dynamic Typing inherits from Suggest Types when left unchecked.
          </div>
        </div>
      )}

      {mode === 'json' && (
        <div
          data-tour="project-import-data-overview"
          className={cn('flex h-[70vh] min-h-[300px] flex-col overflow-hidden pb-5', {
            'opacity-0': loading
          })}
        >
          <Suspense>
            <Editor
              onMount={handleEditorMount}
              defaultLanguage="json"
              defaultValue={defaultValue}
              height="100%"
              onChange={(v) => $editorData.set(v ?? '')}
              theme="vs-dark"
            />
          </Suspense>
        </div>
      )}
      {mode === 'csv' && (
        <div className="mb-5 flex h-[60vh] flex-col">
          <textarea
            className="scrollbar-thin bg-secondary h-full w-full resize-none rounded border p-3 font-mono text-sm outline-none focus-visible:ring"
            placeholder="name,email,age\nJohn Doe,john@example.com,30"
            value={csvData}
            onChange={(e) => $csvData.set(e.target.value)}
          />
        </div>
      )}
      {(loading || submitting || csvSubmitting) && <DialogLoadingOverlay />}
      <DialogFooter>
        <Button onClick={() => $step.set('method')} variant="secondary">
          Back
        </Button>
        {mode === 'json' && (
          <Button
            data-tour="project-import-data-ingest"
            onClick={() => {
              if (!label) {
                setError('Label is required')
                return
              }
              mutate({
                data: JSON.parse($editorData.get()),
                label,
                options: {
                  suggestTypes,
                  castNumberArraysToVectors,
                  convertNumericValuesToNumbers,
                  capitalizeLabels,
                  relationshipType
                }
              }).then(() => {
                $router.open(getRoutePath('project', { id: projectId! }))
                $step.set('method')
              })
            }}
            loading={submitting}
            variant="accent"
          >
            Import JSON
          </Button>
        )}
        {mode === 'csv' && (
          <Button
            onClick={() => {
              if (!label) {
                setError('Label is required')
                return
              }
              mutateCsv({
                label,
                data: csvData,
                options: {
                  suggestTypes,
                  castNumberArraysToVectors,
                  convertNumericValuesToNumbers,
                  capitalizeLabels,
                  relationshipType
                },
                parseConfig: {
                  delimiter,
                  header,
                  skipEmptyLines,
                  quoteChar,
                  escapeChar,
                  newline: newline || undefined,
                  dynamicTyping: dynamicTyping
                }
              }).then(() => {
                $router.open(getRoutePath('project', { id: projectId! }))
                $step.set('method')
              })
            }}
            loading={csvSubmitting}
            variant="accent"
          >
            Import CSV
          </Button>
        )}
      </DialogFooter>
    </>
  )
}

type IngestModalSteps = 'editor' | 'method'

function isNDJSONorJSON(input: string): 'NDJSON' | 'JSON' | 'Unknown' {
  try {
    // Try parsing as JSON first
    JSON.parse(input)
    return 'JSON'
  } catch (jsonError) {
    // If JSON parsing fails, check if it's NDJSON
    const lines = input.split('\n').filter((line) => line.trim() !== '')

    for (const line of lines) {
      try {
        JSON.parse(line)
      } catch (ndjsonError) {
        return 'Unknown'
      }
    }

    return 'NDJSON'
  }
}

export function ImportRecords() {
  const page = useStore($router)
  const step = useStore($step)

  useEffect(() => {
    if (page?.route === 'projectImportData') {
      setTourStep('projectImportRadio', false)
    }
  }, [page?.route])

  const handleJsonUploadChange = (e: ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return

    try {
      const fileReader = new FileReader()
      fileReader.readAsText(file as Blob)

      fileReader.onload = (e) => {
        const result = e.target?.result as string

        const format = isNDJSONorJSON(result)

        if (format === 'NDJSON') {
          // Convert NDJSON to JSON
          $editorData.set(
            '[' +
              result
                .trim()
                .split('\n')
                .map((line) => line)
                .join() +
              ']'
          )
        } else if (format === 'JSON') {
          // Parse as normal JSON
          $editorData.set(result)
        } else {
          throw new Error('File content cannot be recognized as JSON or NDJSON')
        }
      }
    } catch (error) {
      console.error('Error reading the file:', error)
    }
  }

  return (
    <div className={cn({ 'sm:max-w-none': step === 'editor' })}>
      <PageHeader contained>
        <div className="flex gap-3">
          <DatabaseZap />
          <PageTitle>Import data</PageTitle>
        </div>
      </PageHeader>
      <div className="container">
        {step === 'method' && (
          <RadioGroup className="mt-5">
            <RadioButton
              onClick={async () => {
                try {
                  const data = await import('../batchData.json').then((mod) => mod.default)
                  $editorData.set(JSON.stringify(data))
                  $label.set('COMPANY')
                  setTourStep('projectImportOverview', true)
                } catch (error) {}
              }}
              description="We'll upload a test dataset for you to explore."
              icon={<TestTube2 />}
              title="Use test dataset"
              data-tour="project-import-data-radio"
            />
            <RadioButton
              description="Create a new json from scratch"
              icon={<Edit />}
              onClick={() => {
                $editorData.set(`{}`)
                $label.set('')
                $mode.set('json')
              }}
              title="Write from scratch"
            />
            <div className="group relative flex">
              <RadioButton
                description={
                  <>
                    Upload your own JSON or NDJSON file of any structure.
                    <br /> You will get a chance to edit it.
                  </>
                }
                icon={<Braces />}
                title="Upload .json or .ndjson file"
              />
              <input
                onChange={handleJsonUploadChange}
                accept=".json,.ndjson"
                className="absolute left-0 top-0 h-full w-full cursor-pointer appearance-none opacity-0"
                multiple={false}
                type="file"
              />
            </div>
            <div className="group relative flex">
              <RadioButton
                description={<>Upload a CSV file (header row optional). Configure parsing and import.</>}
                icon={<Table />}
                title="Upload .csv file"
                onClick={() => {
                  $mode.set('csv')
                  $csvData.set('')
                  $step.set('editor')
                }}
              />
              <input
                onChange={(e: ChangeEvent<HTMLInputElement>) => {
                  const file = e.target.files?.[0]
                  if (!file) return
                  const reader = new FileReader()
                  reader.onload = (ev) => {
                    $csvData.set((ev.target?.result as string) || '')
                    $mode.set('csv')
                    $step.set('editor')
                  }
                  reader.readAsText(file)
                }}
                accept=".csv"
                className="absolute left-0 top-0 h-full w-full cursor-pointer appearance-none opacity-0"
                multiple={false}
                type="file"
              />
            </div>
          </RadioGroup>
        )}

        {step === 'editor' && <EditorStep />}
      </div>
    </div>
  )
}
