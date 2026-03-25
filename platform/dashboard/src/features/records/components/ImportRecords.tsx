import { useStore } from '@nanostores/react'
import { Braces, DatabaseZap, Edit, TestTube2, Settings2, UploadCloud } from 'lucide-react'
import { atom, onSet } from 'nanostores'
import { type ReactNode, Suspense, lazy, useState, ChangeEvent, useEffect, useRef } from 'react'

import { Button } from '~/elements/Button'
import { DialogLoadingOverlay } from '~/elements/Dialog'
import { TextField } from '~/elements/Input'
import { cn } from '~/lib/utils'

import { useImportCsvMutation, useImportJsonMutation } from '../hooks/useRecordMutations'
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

function formatEstimatedSize(content: string): string {
  const bytes = new TextEncoder().encode(content || '').length

  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`
}

function EditorStep() {
  const [loading, setLoading] = useState(true)
  const defaultValue = useStore($editorData)
  const { mutateAsync: mutate, isPending: submitting } = useImportJsonMutation()
  const { mutateAsync: mutateCsv, isPending: csvSubmitting } = useImportCsvMutation()
  const projectId = useStore($currentProjectId)
  const mode = useStore($mode)
  const csvData = useStore($csvData)
  const editorData = useStore($editorData)
  const currentSourceContent = mode === 'json' ? editorData : csvData
  const sourceSizeEstimate = formatEstimatedSize(currentSourceContent)

  const [suggestTypes, setSuggestTypes] = useState(true)
  const [convertNumericValuesToNumbers, setConvertNumericValuesToNumbers] = useState(false)
  const [mergeMode, setMergeMode] = useState(false)
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
    <div className="h-[calc(100dvh-262px+57px)] overflow-hidden">
      <div className="grid h-full grid-cols-1 gap-4 xl:grid-cols-[3fr_1fr]">
        <section className="bg-fill2 relative flex h-full min-h-0 flex-col overflow-hidden rounded-xl border">
          <div className="border-border bg-surface-secondary flex items-center justify-between border-b px-4 py-2">
            <p className="text-sm font-semibold">Source Data</p>
            <p className="text-content2 text-sm uppercase tracking-wide">
              {mode === 'json' ? 'JSON' : 'CSV'} | ~{sourceSizeEstimate}
            </p>
          </div>

          {mode === 'json' && (
            <div
              data-tour="project-import-data-overview"
              className={cn('flex min-h-0 flex-1 flex-col overflow-hidden p-3', {
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
            <div className="flex min-h-0 flex-1 flex-col p-3">
              <textarea
                className="scrollbar-thin bg-secondary h-full w-full resize-none rounded border p-3 font-mono text-sm outline-none focus-visible:ring"
                placeholder="name,email,age\nJohn Doe,john@example.com,30"
                value={csvData}
                onChange={(e) => $csvData.set(e.target.value)}
              />
            </div>
          )}
        </section>

        <aside className="flex h-full min-h-0 flex-col overflow-hidden">
          <div className="pb-2">
            <h3 className="text-md font-semibold">Import Configuration</h3>
            <p className="text-content2 text-sm">Tune parsing and ingestion behavior before submitting.</p>
          </div>

          <div className="scrollbar-thin flex-1 space-y-4 overflow-y-auto py-3">
            <div>
              <TextField
                required={true}
                label="Label *"
                caption={
                  mode === 'json' ?
                    'Top-level label for created records from this JSON payload.'
                  : 'Label applied to records created from each CSV row.'
                }
                onChange={(event: { target: { value: string } }) => {
                  setError(undefined)
                  $label.set(event.target.value)
                }}
                size="small"
                value={label}
                error={error}
              />
            </div>

            <div>
              <TextField
                label="Relationship Type"
                caption="Used for links between nested records; defaults to platform relation type."
                onChange={(event: { target: { value: string } }) => {
                  setRelationshipType(event.target.value)
                }}
                size="small"
                value={relationshipType}
              />
            </div>

            <div className="space-y-4">
              <div className="space-y-1">
                <CheckboxField
                  className="justify-end"
                  label="Suggest data types"
                  onCheckedChange={() => {
                    setSuggestTypes(!suggestTypes)
                  }}
                  checked={suggestTypes}
                />
                <p className="text-content2 mt-1 text-sm">
                  Infers number, boolean, and null values from input.
                </p>
              </div>
              <div className="space-y-1">
                <CheckboxField
                  className="justify-end"
                  label="Capitalize labels"
                  onCheckedChange={() => {
                    setCapitalizeLabels(!capitalizeLabels)
                  }}
                  checked={capitalizeLabels}
                />
                <p className="text-content2 mt-1 text-sm">Normalizes labels to uppercase for consistency.</p>
              </div>
              <div className="space-y-1">
                <CheckboxField
                  className="justify-end"
                  label="Cast numeric values to numbers"
                  onCheckedChange={() => {
                    setConvertNumericValuesToNumbers(!convertNumericValuesToNumbers)
                  }}
                  checked={convertNumericValuesToNumbers}
                />
                <p className="text-content2 mt-1 text-sm">
                  Converts numeric-looking strings into numeric properties.
                </p>
              </div>
              <div className="space-y-1">
                <CheckboxField
                  className="justify-end"
                  label="Merge mode for upsert imports"
                  onCheckedChange={() => {
                    setMergeMode(!mergeMode)
                  }}
                  checked={mergeMode}
                />
                <p className="text-content2 mt-1 text-sm">
                  Updates matching records instead of always creating new ones.
                </p>
              </div>
            </div>

            {mode === 'csv' && (
              <div className="rounded border p-3">
                <div className="mb-3 flex items-center gap-2 font-semibold">
                  <Settings2 size={16} /> CSV Parse Config
                </div>
                <div className="space-y-3">
                  <TextField
                    label="Delimiter"
                    caption="Column separator, usually comma, semicolon, or tab."
                    size="small"
                    value={delimiter}
                    onChange={(e: any) => setDelimiter(e.target.value)}
                  />
                  <TextField
                    label="Quote Char"
                    caption="Character wrapping values that contain delimiters."
                    size="small"
                    value={quoteChar}
                    onChange={(e: any) => setQuoteChar(e.target.value)}
                  />
                  <TextField
                    label="Escape Char"
                    caption="Character used to escape quote characters in values."
                    size="small"
                    value={escapeChar}
                    onChange={(e: any) => setEscapeChar(e.target.value)}
                  />
                  <TextField
                    label="Newline"
                    caption="Leave empty to auto-detect line breaks."
                    size="small"
                    placeholder="auto"
                    value={newline ?? ''}
                    onChange={(e: any) => setNewline(e.target.value || undefined)}
                  />

                  <div className="space-y-1">
                    <CheckboxField
                      className="justify-end"
                      label="Header Row"
                      checked={header}
                      onCheckedChange={() => setHeader(!header)}
                    />
                    <p className="text-content2 mt-1 text-sm">Treats first row as column names.</p>
                  </div>
                  <div className="space-y-1">
                    <CheckboxField
                      className="justify-end"
                      label="Skip Empty"
                      checked={skipEmptyLines}
                      onCheckedChange={() => setSkipEmptyLines(!skipEmptyLines)}
                    />
                    <p className="text-content2 mt-1 text-sm">Ignores blank lines while parsing.</p>
                  </div>
                  <div className="space-y-1">
                    <CheckboxField
                      className="justify-end"
                      label="Dynamic Typing"
                      checked={dynamicTyping ?? false}
                      onCheckedChange={() => setDynamicTyping(dynamicTyping ? undefined : true)}
                    />
                    <p className="text-content2 mt-1 text-sm">
                      Overrides type inference for CSV parser. When off, it follows Suggest data types.
                    </p>
                  </div>
                </div>
              </div>
            )}
          </div>

          <div className="border-border flex items-center justify-end gap-2 border-t px-4 py-3">
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
                      convertNumericValuesToNumbers,
                      capitalizeLabels,
                      relationshipType,
                      mergeBy: mergeMode ? [] : undefined
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
                      convertNumericValuesToNumbers,
                      capitalizeLabels,
                      relationshipType,
                      mergeBy: mergeMode ? [] : undefined
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
          </div>
        </aside>
      </div>

      {(loading || submitting || csvSubmitting) && <DialogLoadingOverlay />}
    </div>
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

type SupportedImportFileType = 'json' | 'jsonl' | 'ndjson' | 'csv' | 'unknown'

function parseJsonLines(input: string): string {
  const lines = input
    .replace(/^\uFEFF/, '')
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0)

  if (!lines.length) {
    throw new Error('The uploaded JSONL/NDJSON file is empty.')
  }

  const records = lines.map((line, index) => {
    try {
      return JSON.parse(line)
    } catch {
      throw new Error(`Invalid JSON line at ${index + 1}.`)
    }
  })

  return JSON.stringify(records, null, 2)
}

function looksLikeCsv(input: string): boolean {
  const lines = input
    .replace(/^\uFEFF/, '')
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0)

  if (lines.length < 2) {
    return false
  }

  const firstLine = lines[0]
  const delimiters = [',', ';', '\t']

  return delimiters.some((delimiter) => firstLine.includes(delimiter))
}

function detectImportFileType(fileName: string, input: string): SupportedImportFileType {
  const extension = fileName.split('.').pop()?.toLowerCase()

  if (extension === 'csv') return 'csv'
  if (extension === 'jsonl') return 'jsonl'
  if (extension === 'ndjson') return 'ndjson'
  if (extension === 'json') return 'json'

  const normalized = input.replace(/^\uFEFF/, '').trim()
  if (!normalized) {
    return 'unknown'
  }

  try {
    JSON.parse(normalized)
    return 'json'
  } catch {
    const ndjsonOrJson = isNDJSONorJSON(normalized)
    if (ndjsonOrJson === 'NDJSON') {
      return 'jsonl'
    }
    if (looksLikeCsv(normalized)) {
      return 'csv'
    }
    return 'unknown'
  }
}

export function ImportRecords() {
  const page = useStore($router)
  const step = useStore($step)
  const [isDraggingFile, setIsDraggingFile] = useState(false)
  const [uploadError, setUploadError] = useState<string | null>(null)
  const [lastDetectedType, setLastDetectedType] = useState<string | null>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (page?.route === 'projectImportData') {
      setTourStep('projectImportRadio', false)
    }
  }, [page?.route])

  useEffect(() => {
    if (step === 'method') {
      setLastDetectedType(null)
      setUploadError(null)
      setIsDraggingFile(false)
    }
  }, [step])

  const processImportFile = (file: File) => {
    const fileReader = new FileReader()

    fileReader.onload = (event) => {
      try {
        const raw = (event.target?.result as string) || ''
        const content = raw.replace(/^\uFEFF/, '')
        const detectedType = detectImportFileType(file.name, content)

        if (detectedType === 'unknown') {
          throw new Error('Unsupported file type. Please upload .json, .jsonl, .ndjson, or .csv.')
        }

        setUploadError(null)
        setLastDetectedType(detectedType.toUpperCase())

        if (detectedType === 'csv') {
          $csvData.set(content)
          $mode.set('csv')
          $step.set('editor')
          return
        }

        if (detectedType === 'jsonl' || detectedType === 'ndjson') {
          const normalized = parseJsonLines(content)
          $editorData.set(normalized)
          $mode.set('json')
          $step.set('editor')
          return
        }

        const parsed = JSON.parse(content)
        if (typeof parsed !== 'object' || parsed === null) {
          throw new Error('JSON import supports only object or array payloads.')
        }

        $editorData.set(JSON.stringify(parsed, null, 2))
        $mode.set('json')
        $step.set('editor')
      } catch (error: any) {
        setLastDetectedType(null)
        setUploadError(error?.message || 'Could not parse uploaded file.')
      }
    }

    fileReader.onerror = () => {
      setLastDetectedType(null)
      setUploadError('Unable to read selected file.')
    }

    fileReader.readAsText(file)
  }

  const handleFileInputChange = (e: ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return
    processImportFile(file)
    e.target.value = ''
  }

  const openFilePicker = () => {
    fileInputRef.current?.click()
  }

  return (
    <div
      className={cn('flex min-h-0 flex-1 flex-col', { 'overflow-hidden sm:max-w-none': step === 'editor' })}
    >
      <PageHeader contained>
        <div className="flex gap-3">
          <DatabaseZap />
          <PageTitle>Import data</PageTitle>
        </div>
      </PageHeader>
      <div className={cn('container', step === 'editor' && 'max-w-none overflow-hidden px-2 sm:px-4')}>
        {step === 'method' && (
          <div className="mt-6">
            <input
              ref={fileInputRef}
              onChange={handleFileInputChange}
              accept=".json,.jsonl,.ndjson,.csv"
              className="hidden"
              multiple={false}
              type="file"
            />

            <div
              className={cn(
                'border-border relative flex w-full flex-col items-center justify-center gap-3 rounded-xl border border-dashed bg-transparent px-6 py-12 text-center transition-all',
                isDraggingFile && 'border-accent bg-accent/5 ring-accent/30 ring-2'
              )}
              onDragOver={(event) => {
                event.preventDefault()
                setIsDraggingFile(true)
              }}
              onDragLeave={(event) => {
                event.preventDefault()
                setIsDraggingFile(false)
              }}
              onDrop={(event) => {
                event.preventDefault()
                setIsDraggingFile(false)
                const file = event.dataTransfer.files?.[0]
                if (!file) return
                processImportFile(file)
              }}
            >
              <div className="bg-accent/15 text-accent flex h-14 w-14 items-center justify-center rounded-full">
                <UploadCloud size={24} />
              </div>
              <div>
                <p className="text-lg font-semibold">Drag and drop your import file</p>
                <p className="text-content2 mt-1 text-sm">
                  Auto-detects JSON, JSONL, NDJSON, and CSV, then routes you to the right import flow.
                </p>
              </div>
              <Button onClick={openFilePicker} variant="outline" size="small" className="mt-2">
                Browse files
              </Button>
              {lastDetectedType && (
                <p className="text-content2 text-sm">Detected format: {lastDetectedType}</p>
              )}
              {uploadError && <p className="text-danger text-sm">{uploadError}</p>}
            </div>

            <div className="mt-5 grid grid-cols-1 gap-4 md:grid-cols-2">
              <RadioButton
                onClick={async () => {
                  try {
                    const data = await import('../batchData.json').then((mod) => mod.default)
                    $editorData.set(JSON.stringify(data, null, 2))
                    $label.set('COMPANY')
                    $mode.set('json')
                    setTourStep('projectImportOverview', true)
                  } catch (error) {}
                }}
                description="We'll upload a test dataset for you to explore."
                icon={<TestTube2 />}
                title="Use test dataset"
                data-tour="project-import-data-radio"
              />
              <RadioButton
                description="Create a new JSON payload from scratch."
                icon={<Edit />}
                onClick={() => {
                  $editorData.set(`{}`)
                  $label.set('')
                  $mode.set('json')
                }}
                title="Write from scratch"
              />
              <RadioButton
                description="Open the file picker and import a data file with automatic format detection."
                icon={<Braces />}
                onClick={openFilePicker}
                title="Upload structured data"
              />
              <RadioButton
                description="Jump straight into CSV mode and paste or edit rows manually."
                icon={<UploadCloud />}
                onClick={() => {
                  $mode.set('csv')
                  $csvData.set('')
                  $step.set('editor')
                }}
                title="Start with CSV editor"
              />
            </div>
          </div>
        )}

        {step === 'editor' && <EditorStep />}
      </div>
    </div>
  )
}
