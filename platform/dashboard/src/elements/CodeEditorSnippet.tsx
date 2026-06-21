import type { ReactNode } from 'react'

import { CopyButton } from '~/elements/Button'
import { Editor } from '~/elements/Editor'
import { getNumberOfLines } from '~/lib/utils'

export function CodeEditorSnippet({
  code,
  language,
  title
}: {
  code: string
  language: string
  title: ReactNode
}) {
  return (
    <div className="bg-fill overflow-hidden rounded-md border">
      <div className="flex items-center justify-between gap-3 border-b px-3 py-2">
        <span className="text-content2 text-sm font-medium">{title}</span>
        <CopyButton text={code} size="xsmall" variant="outline">
          Copy
        </CopyButton>
      </div>
      <Editor
        key={language}
        value={code}
        height={`${getNumberOfLines(code) * 1.2}em`}
        defaultLanguage={language === 'shell' ? 'shell' : language}
        format={language !== 'shell'}
        readOnly
        lineNumbers="off"
      />
    </div>
  )
}
