import type { ComponentPropsWithoutRef } from 'react'
import { Suspense, lazy, useEffect, useState } from 'react'
import { useStore } from '@nanostores/react'
import { Skeleton } from '~/elements/Skeleton'
import { $resolvedTheme } from '~/features/auth/stores/theme'

const MonacoEditor = lazy(() =>
  import('~/lib/monacoSetup').then(() =>
    import('@monaco-editor/react').then((module) => ({
      default: module.Editor
    }))
  )
)

type MonacoProps = ComponentPropsWithoutRef<typeof MonacoEditor>

type EditorProps = {
  minimap?: boolean
  readOnly?: boolean
  lineNumbers?: 'off' | 'on'
  format?: boolean
} & Pick<
  MonacoProps,
  'height' | 'className' | 'defaultValue' | 'onChange' | 'value' | 'defaultLanguage' | 'theme'
>

export const Editor = ({
  defaultValue,
  value,
  onChange,
  defaultLanguage = 'javascript',
  theme,
  height = '100%',
  minimap = false,
  readOnly = false,
  lineNumbers = 'on',
  format = true,
  className
}: EditorProps) => {
  const [editor, setEditor] = useState<any>(null)
  const resolvedTheme = useStore($resolvedTheme)
  // An explicit `theme` prop wins; otherwise follow the app theme.
  const editorTheme = theme ?? (resolvedTheme === 'light' ? 'light' : 'vs-dark')

  useEffect(() => {
    if (!editor || !format) return

    const formatDocument = async () => {
      if (readOnly) {
        editor.updateOptions({ readOnly: false }) // Disable readOnly before formatting
      }

      const formatAction = editor.getAction('editor.action.formatDocument')
      if (formatAction) {
        await formatAction.run()
      }

      if (readOnly) {
        editor.updateOptions({ readOnly: true }) // Re-enable readOnly after formatting
      }
    }

    const t = setTimeout(formatDocument, 100)

    return () => clearTimeout(t)
  }, [editor, value, readOnly, format])

  return (
    <Suspense fallback={<Skeleton className="w-full" style={{ height }} enabled />}>
      <MonacoEditor
        onMount={(editor) => {
          setEditor(editor)
          editor.getAction('editor.action.formatDocument')?.run()

          editor.onDidBlurEditorWidget(() => {
            editor.getAction('editor.action.formatDocument')?.run()
          })

          editor.onDidFocusEditorWidget(() => {
            editor.getAction('editor.action.formatDocument')?.run()
          })
        }}
        defaultLanguage={defaultLanguage}
        value={value}
        defaultValue={defaultValue}
        height={height}
        onChange={onChange}
        theme={editorTheme}
        className={className}
        options={{
          readOnly: readOnly,
          minimap: {
            enabled: minimap
          },
          maxTokenizationLineLength: 100000, // Increase the tokenization limit
          lineNumbers
        }}
      />
    </Suspense>
  )
}
