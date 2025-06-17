import { CSSProperties, PropsWithoutRef, forwardRef, ReactNode, useContext, useCallback } from 'react'

import { CodeBlock } from '~/components/CodeBlock'
import Image from 'next/image'

import cn from 'classnames'
import { CodingLanguage } from '~/contexts/CodingLanguage'

type CodeBlockWithLanguageSelectorProps = {
  data: Record<string, string>
  className?: string
  wrapperClassName?: string
  preClassName?: string
  style?: CSSProperties
  children?: ReactNode
  copyButton?: boolean
}

const PyLogo = ({ className }: { className: string }) => (
  <Image
    src="/images/python-logo.svg"
    alt="Python Logo"
    width={32}
    height={32}
    className={className}
    priority
  />
)
const TsLogo = ({ className }: { className: string }) => (
  <Image
    priority
    src="/images/typescript-logo.svg"
    alt="Typescript Logo"
    width={32}
    height={32}
    className={className}
  />
)

const langLogoMap = {
  python: PyLogo,
  typescript: TsLogo
}

const Logo = ({ lang, className }: { lang: string; className: string }) => {
  // @ts-ignore
  const Comp = langLogoMap[lang]

  return <Comp className={className} />
}

export const CodeBlockWithLanguageSelector = forwardRef<
  HTMLDivElement,
  PropsWithoutRef<CodeBlockWithLanguageSelectorProps>
>(({ data, className, preClassName, wrapperClassName, children: extra, copyButton, style }, ref) => {
  const { language, setLanguage: setLang } = useContext(CodingLanguage)

  const setLanguage = useCallback((lang: string) => setLang(lang), [language])

  return (
    <div className={cn(className, 'w-full rounded-xl bg-[#131313]')} ref={ref} style={style}>
      <div className="text-content-contrast border-b-stroke mb-2 flex w-full cursor-pointer justify-between rounded-t-xl border-b border-b-[#404040] bg-[#131313] text-center font-mono font-bold dark:bg-[#0a0a0a]">
        {Object.keys(data).map((key) => (
          <div
            key={key}
            onClick={() => setLanguage(key)}
            className={cn('flex w-full grow items-center justify-center py-2 transition hover:grayscale-0', {
              grayscale: key !== language,
              'dark:border-content border-b-2 text-white dark:text-white': key === language,
              'text-content3 dark:text-content2': key !== language
            })}
          >
            <Logo className="h-9 w-9 p-2" lang={key} /> {key}
          </div>
        ))}
      </div>

      <CodeBlock
        code={data[language]}
        className={className}
        preClassName={preClassName}
        language={language}
        wrapperClassName={cn(wrapperClassName, 'rounded-xl rounded-t-none ')}
        copyButton={copyButton}
      >
        {extra}
      </CodeBlock>
    </div>
  )
})

CodeBlockWithLanguageSelector.displayName = 'CodeBlockWithLanguageSelector'
