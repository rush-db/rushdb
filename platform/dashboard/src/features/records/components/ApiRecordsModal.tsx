import { useStore } from '@nanostores/react'
import { Code2 } from 'lucide-react'
import { atom } from 'nanostores'
import React, { Suspense, lazy, useState } from 'react'

import { Button } from '~/elements/Button'
import { Dialog, DialogTitle } from '~/elements/Dialog'

import { cn } from '~/lib/utils'
import { SelectSdkLanguage } from '~/features/onboarding/components/SelectSdkLanguage.tsx'
import { $settings } from '~/features/auth/stores/settings.ts'
import { $editorData } from '~/features/projects/stores/raw-api.ts'

const Editor = lazy(() =>
  import('@monaco-editor/react').then((module) => ({
    default: module.Editor
  }))
)

const $open = atom<boolean>(false)
const $codeData = atom<string>('')

function CodeSnippet() {
  const [loading, setLoading] = useState(true)

  const { sdkLanguage } = useStore($settings)
  const editorData = useStore($editorData)

  return (
    <>
      <div
        className={cn('mt-5 flex h-[70vh] min-h-[300px] flex-col overflow-hidden', {
          'opacity-0': loading
        })}
      >
        <div className="my-5 flex w-full items-center justify-between">
          <p className="text-content2 text-lg">Use this code in your application</p>
          <div className="flex items-center">
            <SelectSdkLanguage />
          </div>
        </div>

        <Suspense>
          <Editor
            onMount={(editor) => {
              setTimeout(function () {
                editor
                  .getAction('editor.action.formatDocument')
                  ?.run()
                  .then(() => setLoading(false))
              }, 100)
              editor.getAction('editor.action.formatDocument')?.run()
            }}
            defaultLanguage={sdkLanguage}
            value={sdkLanguage === 'python' ? PyTemplate(editorData!) : JSTemplate(editorData!)}
            height="100%"
            onChange={(v) => $codeData.set(v ?? '')}
            theme="vs-dark"
          />
        </Suspense>
      </div>
    </>
  )
}

const JSTemplate = (body: string) => `import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('<YOUR_API_KEY>')

await db.records.find(${body})`

const PyTemplate = (body: string) => `from rushdb import RushDB

db = RushDB("RUSHDB_API_KEY")

db.records.find(${body})`

export function ApiRecordsModal() {
  const open = useStore($open)

  return (
    <Dialog
      trigger={
        <Button size="small" variant="secondary">
          <Code2 className="text-accent" /> <span className="font-mono">API</span>
        </Button>
      }
      className={cn('sm:max-w-4xl')}
      onOpenChange={$open.set}
      open={open}
    >
      <DialogTitle>
        <Code2 className="text-accent" />
        API
      </DialogTitle>

      <CodeSnippet />
    </Dialog>
  )
}
