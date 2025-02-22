import { CSSProperties, PropsWithoutRef, forwardRef, ReactNode, Children } from 'react'
import { materialDark as codeTheme } from 'react-syntax-highlighter/dist/cjs/styles/prism'
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter'
import cx from 'classnames'

import { CopyButton } from '~/components/CopyButton'

const PreStyles: CSSProperties = {
  textAlign: 'left',
  whiteSpace: 'pre',
  wordSpacing: 'normal',
  wordBreak: 'normal',
  overflowWrap: 'normal',
  color: 'rgb(195, 206, 227)',
  background: '#131313',
  fontSize: '1em',
  hyphens: 'none',
  overflow: 'auto',
  position: 'relative'
}

const override = {
  ...codeTheme,
  'code[class*="language-"]': {
    ...codeTheme['code[class*="language-"]'],
    background: 'inherit',
    fontFamily: 'var(--font-jet-brains-mono) !important'
  }
}

export const CodeBlock = forwardRef<
  HTMLDivElement,
  PropsWithoutRef<{
    code: string
    className?: string
    wrapperClassName?: string
    preClassName?: string
    style?: CSSProperties
    showLineNumbers?: boolean
    language?: string
    children?: ReactNode
    copyButton?: boolean
  }>
>(
  (
    {
      code,
      className,
      showLineNumbers = false,
      preClassName,
      wrapperClassName,
      children: extra,
      copyButton,
      style,
      language = 'javascript'
    },
    ref
  ) => {
    return (
      <div className={cx('sm:text-[14px]', className)} ref={ref} style={style}>
        <SyntaxHighlighter
          language={language}
          style={override}
          showLineNumbers={showLineNumbers}
          PreTag={({ children }) => (
            <div
              className={cx(wrapperClassName, 'flex items-start justify-between gap-2 rounded-xl')}
              style={PreStyles}
            >
              <pre className={cx(preClassName, 'p-4')}>{children}</pre>
              {Children.count(extra) || copyButton ?
                <div className="flex h-full items-center gap-4 pr-4">
                  {extra}
                  {copyButton && <CopyButton text={code} />}
                </div>
              : null}
            </div>
          )}
        >
          {code}
        </SyntaxHighlighter>
      </div>
    )
  }
)

CodeBlock.displayName = 'CodeBlock'
