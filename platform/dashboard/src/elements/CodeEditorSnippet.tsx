import type { ReactNode } from 'react'

import { CopyButton } from '~/elements/Button'
import { Editor } from '~/elements/Editor'
import { getNumberOfLines } from '~/lib/utils'

export function CodeEditorSnippet({
  code,
  language,
  title,
  height
}: {
  code: string
  language: string
  title: ReactNode
  /** Fixed editor height; defaults to fitting the snippet's line count. */
  height?: string
}) {
  return (
    <div className="overflow-hidden rounded-md border bg-fill">
      <div className="flex items-center justify-between gap-3 border-b px-3 py-2">
        <span className="text-sm font-medium text-content2">{title}</span>
        <CopyButton text={code} size="xsmall" variant="outline">
          Copy
        </CopyButton>
      </div>
      <Editor
        key={language}
        value={code}
        height={height ?? `${getNumberOfLines(code) * 1.2}em`}
        defaultLanguage={language === 'shell' ? 'shell' : language}
        format={language !== 'shell'}
        readOnly
        lineNumbers="off"
      />
    </div>
  )
}
