import { CSSProperties, PropsWithoutRef, forwardRef, ReactNode, Children, useState, useEffect } from 'react'
import { materialDark } from 'react-syntax-highlighter/dist/cjs/styles/prism'
import { materialLight } from 'react-syntax-highlighter/dist/cjs/styles/prism'
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter'
import cx from 'classnames'

import { CopyButton } from '~/components/CopyButton'

// Dark theme overrides
const darkPreStyles: CSSProperties = {
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

// Light theme overrides — warm beige to match LP surface
const lightPreStyles: CSSProperties = {
  ...darkPreStyles,
  background: '#F0EDE5',
  color: '#3a3530'
}

const darkOverride = {
  ...materialDark,
  'code[class*="language-"]': {
    ...materialDark['code[class*="language-"]'],
    background: 'inherit',
    fontFamily: 'var(--font-jet-brains-mono) !important'
  }
}

const lightOverride = {
  ...materialLight,
  'code[class*="language-"]': {
    ...materialLight['code[class*="language-"]'],
    background: 'inherit',
    color: '#3a3530',
    fontFamily: 'var(--font-jet-brains-mono) !important'
  },
  comment: { ...materialLight['comment'], color: '#9a8f85' },
  string: { ...materialLight['string'], color: '#C8540A' },
  keyword: { ...materialLight['keyword'], color: '#7c3aed' },
  number: { ...materialLight['number'], color: '#b45309' },
  function: { ...materialLight['function'], color: '#0369a1' },
  operator: { ...(materialLight['operator'] ?? {}), color: '#8b5c3a' }
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
    const [isClient, setIsClient] = useState(false)
    const [isDark, setIsDark] = useState(true)

    useEffect(() => {
      setIsClient(true)
      const sync = () => setIsDark(document.documentElement.classList.contains('dark'))
      sync()
      const mo = new MutationObserver(sync)
      mo.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
      return () => mo.disconnect()
    }, [])

    if (!isClient) {
      return <div className="text-content3 text-sm italic"></div>
    }

    const preStyles = isDark ? darkPreStyles : lightPreStyles
    const themeOverride = isDark ? darkOverride : lightOverride

    return (
      <div className={cx('sm:text-[14px]', className, 'h-full')} ref={ref} style={style}>
        <SyntaxHighlighter
          language={language}
          style={themeOverride}
          showLineNumbers={showLineNumbers}
          PreTag={({ children }) => (
            <div
              className={cx(wrapperClassName, 'flex h-full items-start justify-between gap-2')}
              style={preStyles}
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
