import {
  ComponentPropsWithoutRef,
  Suspense,
  lazy,
  useEffect,
  useState
} from 'react'
import { Skeleton } from '~/elements/Skeleton'

const MonacoEditor = lazy(() =>
  import('@monaco-editor/react').then((module) => ({
    default: module.Editor
  }))
)

type MonacoProps = ComponentPropsWithoutRef<typeof MonacoEditor>

type EditorProps = {
  minimap?: boolean
  readOnly?: boolean
  lineNumbers?: 'off' | 'on'
} & Pick<
  MonacoProps,
  | 'height'
  | 'className'
  | 'defaultValue'
  | 'onChange'
  | 'value'
  | 'defaultLanguage'
  | 'theme'
>

export const Editor = ({
  defaultValue,
  value,
  onChange,
  defaultLanguage = 'javascript',
  theme = 'vs-dark',
  height = '100%',
  minimap = false,
  readOnly = false,
  lineNumbers = 'on',
  className
}: EditorProps) => {
  const [editor, setEditor] = useState<any>(null)

  useEffect(() => {
    let t = setTimeout(() => {
      if (editor) {
        editor?.getAction('editor.action.formatDocument')?.run()
      }
    }, 100)

    return () => clearTimeout(t)
  }, [editor, value])

  return (
    <Suspense
      fallback={<Skeleton className="w-full" style={{ height }} enabled />}
    >
      <MonacoEditor
        onMount={(editor) => {
          setEditor(editor)
        }}
        defaultLanguage={defaultLanguage}
        value={value}
        defaultValue={defaultValue}
        height={height}
        onChange={onChange}
        theme={theme}
        className={className}
        options={{
          readOnly: readOnly,
          minimap: {
            enabled: minimap
          },
          lineNumbers
        }}
      />
    </Suspense>
  )
}
