import { CSSProperties, PropsWithoutRef, forwardRef } from "react"
import {
  materialDark as codeTheme,
  // atomDark,
  // nightOwl,
  // synthwave84,
  // tomorrow,
} from "react-syntax-highlighter/dist/cjs/styles/prism"
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter"
import classNames from "classnames"
import cx from "classnames"

const PreStyles: CSSProperties = {
  textAlign: "left",
  whiteSpace: "pre",
  wordSpacing: "normal",
  wordBreak: "normal",
  overflowWrap: "normal",
  color: "rgb(195, 206, 227)",
  background: "#131313",
  fontSize: "1em",
  hyphens: "none",
  overflow: "auto",
  position: "relative",
}

const override = {
  ...codeTheme,
  'code[class*="language-"]': {
    ...codeTheme['code[class*="language-"]'],
    background: "inherit",
    fontFamily: "var(--font-jet-brains-mono) !important",
  },
}

export const CodeBlock = forwardRef<
  HTMLDivElement,
  PropsWithoutRef<{
    code: string
    className?: string
    preClassName?: string
    style?: CSSProperties
    language?: string
  }>
>(({ code, className, preClassName, style, language = "javascript" }, ref) => {
  return (
    <div
      className={classNames("sm:text-[14px]", className)}
      ref={ref}
      style={style}
    >
      <SyntaxHighlighter
        language={language}
        style={override}
        PreTag={({ children }) => (
          <pre className={cx(preClassName, "p-4 rounded-xl")} style={PreStyles}>
            {children}
          </pre>
        )}
      >
        {code}
      </SyntaxHighlighter>
    </div>
  )
})

CodeBlock.displayName = "CodeBlock"
