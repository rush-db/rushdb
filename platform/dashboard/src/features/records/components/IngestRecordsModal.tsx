import { useStore } from '@nanostores/react'
import { Braces, DatabaseZap, Edit, TestTube2, Upload } from 'lucide-react'
import { atom, onSet } from 'nanostores'
import { type ReactNode, Suspense, lazy, useState, ChangeEvent } from 'react'

import { Button } from '~/elements/Button'
import {
  Dialog,
  DialogFooter,
  DialogLoadingOverlay,
  DialogTitle
} from '~/elements/Dialog'
import { TextField } from '~/elements/Input'
import { cn } from '~/lib/utils'

import { batchUpload } from '../stores/batch'
import { CheckboxField } from '~/elements/Checkbox.tsx'

function RadioGroup({
  className,
  ...props
}: TInheritableElementProps<'div', {}>) {
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
        'flex w-full items-start gap-5 rounded-lg border bg-secondary px-5 py-4 text-start ring-accent-ring transition-all hover:border-accent-hover hover:bg-secondary-hover focus-visible:border-accent-focus focus-visible:ring group-hover:border-accent-hover group-hover:bg-secondary-hover group-focus:ring',
        className
      )}
      {...props}
    >
      <div className="mt-2.5 flex items-center justify-center text-accent">
        {icon}
      </div>
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

const $open = atom<boolean>(false)
const $step = atom<IngestModalSteps>('method')
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
  const { mutate, loading: submitting } = useStore(batchUpload)

  const [suggestTypes, setSuggestTypes] = useState(true)
  const label = useStore($label)

  return (
    <>
      <TextField
        caption="Specify a Label for the top-level Record(s) parsed from this JSON"
        className="mt-5"
        required={true}
        label="Label"
        onChange={(event) => $label.set(event.target.value)}
        size="small"
        value={label}
      />
      <div className="flex gap-5">
        <CheckboxField
          className="mb-5 mt-5"
          label="Suggest data types"
          onCheckedChange={() => {
            setSuggestTypes(!suggestTypes)
          }}
          checked={suggestTypes}
        />
      </div>

      <div
        className={cn(
          '-mx-5 flex h-[70vh] min-h-[300px] flex-col overflow-hidden pb-5',
          {
            'opacity-0': loading
          }
        )}
      >
        <Suspense>
          <Editor
            onMount={(editor) => {
              setTimeout(function () {
                editor
                  .getAction('editor.action.formatDocument')
                  ?.run()
                  .then(() => setLoading(false))
              }, 50)
              // editor.getAction('editor.action.formatDocument')?.run()
            }}
            defaultLanguage="json"
            defaultValue={defaultValue}
            height="100%"
            onChange={(v) => $editorData.set(v ?? '')}
            theme="vs-dark"
          />
        </Suspense>
      </div>
      {(loading || submitting) && <DialogLoadingOverlay />}
      <DialogFooter>
        <Button onClick={() => $step.set('method')} variant="secondary">
          Back
        </Button>
        <Button
          onClick={() => {
            mutate({
              payload: JSON.parse($editorData.get()),
              label,
              options: {
                suggestTypes
              }
            }).then(() => {
              $open.set(false)
              $step.set('method')
            })
          }}
          loading={submitting}
          variant="accent"
        >
          Ingest data
        </Button>
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

export function IngestRecordsModal() {
  const open = useStore($open)
  const step = useStore($step)

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
          throw Error('File content cannot be recognized as JSON or NDJSON')
        }
      }
    } catch (error) {
      console.error('Error reading the file:', error)
    }
  }

  return (
    <Dialog
      trigger={
        <Button size="small" variant="secondary">
          <Upload className="text-green-400" /> Import
          {/*<DatabaseZap />*/}
        </Button>
      }
      className={cn({ 'sm:max-w-none': step === 'editor' })}
      onOpenChange={$open.set}
      open={open}
    >
      <div className="flex gap-3">
        <DatabaseZap />
        <DialogTitle>Import data</DialogTitle>
      </div>

      {step === 'method' && (
        <RadioGroup className="mt-5">
          <RadioButton
            onClick={async () => {
              try {
                const data = await import('../batchData.json').then(
                  (mod) => mod.default
                )
                $editorData.set(JSON.stringify(data))
                $label.set('COMPANY')
              } catch (error) {}
            }}
            description="We'll upload a test dataset for you to explore."
            icon={<TestTube2 />}
            title="Use test dataset"
          />
          <RadioButton
            description="Create a new json from scratch"
            icon={<Edit />}
            onClick={() => {
              $editorData.set(`{}`)
              $label.set('')
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
        </RadioGroup>
      )}

      {step === 'editor' && <EditorStep />}
    </Dialog>
  )
}