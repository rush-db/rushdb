import { useStore } from '@nanostores/react'
import { Code2 } from 'lucide-react'
import { atom } from 'nanostores'
import { Suspense, lazy, useState } from 'react'

import { Button } from '~/elements/Button'
import { Dialog, DialogTitle } from '~/elements/Dialog'
import {
  $activeLabels,
  $combineFilters,
  $currentProjectFilters,
  $currentProjectRecordsLimit,
  $currentProjectRecordsSkip,
  $recordsOrderBy
} from '~/features/projects/stores/current-project'
import { cn } from '~/lib/utils'
import {
  convertToCollectQuery,
  filterToSearchOperation
} from '~/features/projects/utils.ts'

const Editor = lazy(() =>
  import('@monaco-editor/react').then((module) => ({
    default: module.Editor
  }))
)

const $open = atom<boolean>(false)
const $editorData = atom<string>('')

function EditorStep() {
  const [loading, setLoading] = useState(true)
  const defaultValue = useStore($editorData)

  return (
    <>
      <div
        className={cn(
          '-mx-5 mt-5 flex h-[70vh] min-h-[300px] flex-col overflow-hidden pb-5',
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
              }, 100)
              // editor.getAction('editor.action.formatDocument')?.run()
            }}
            defaultLanguage="javascript"
            defaultValue={defaultValue}
            height="100%"
            onChange={(v) => $editorData.set(v ?? '')}
            theme="vs-dark"
          />
        </Suspense>
      </div>
    </>
  )
}

const JSTemplate = (body: string) =>
  `fetch("https://api.collect.so/api/v1/records/search", {
    "headers": {
        "token": "<YOUR_API_KEY>",
        "content-type": "application/json"
    },
    "body": ${body},
    "method": "POST",
    "mode": "cors",
    "credentials": "include"
});`

export function ApiRecordsModal() {
  const open = useStore($open)

  const onClick = () => {
    const filters = $currentProjectFilters.get()
    const sort = $recordsOrderBy.get()
    const skip = $currentProjectRecordsSkip.get()
    const limit = $currentProjectRecordsLimit.get()
    const labels = $activeLabels.get()
    const combineMode = $combineFilters.get()
    const properties = filters.map(filterToSearchOperation)

    const finalQuery = JSON.stringify(
      {
        where:
          combineMode === 'or'
            ? { $or: convertToCollectQuery(properties) }
            : convertToCollectQuery(properties),
        sort,
        skip,
        limit,
        labels
      },
      null,
      4
    )
    $editorData.set(JSTemplate(finalQuery))
  }
  return (
    <Dialog
      trigger={
        <Button onClick={onClick} size="small" variant="secondary">
          <Code2 className="text-accent" />{' '}
          <span className="font-mono">API</span>
        </Button>
      }
      className={cn('sm:max-w-none')}
      onOpenChange={$open.set}
      open={open}
    >
      <DialogTitle>
        <Code2 className="text-accent" />
        API
      </DialogTitle>

      <EditorStep />
    </Dialog>
  )
}
