import { CSSProperties, PropsWithoutRef, forwardRef, ReactNode, useContext } from 'react'

import { CodingLanguage } from '~/pages'
import { CodeBlock } from '~/components/CodeBlock'
import { Tabs, TabsContent, TabsList } from '~/components/Tabs'
import { TabsTrigger } from '@radix-ui/react-tabs'
import cn from 'classnames'

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
  <svg
    width="400"
    height="400"
    viewBox="0 0 400 400"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    className={className}
  >
    <path
      d="M198.563 2C97.0125 2 103.35 46.0375 103.35 46.0375L103.475 91.6625H200.375V105.35H64.9625C64.9625 105.35 0 97.975 0 200.45C0 302.9 56.7125 299.275 56.7125 299.275H90.5625V251.725C90.5625 251.725 88.7375 195.013 146.375 195.013H242.475C242.475 195.013 296.475 195.888 296.475 142.825V55.0875C296.475 55.0875 304.675 2 198.563 2ZM145.125 32.675C154.763 32.675 162.562 40.475 162.562 50.1125C162.562 59.75 154.763 67.55 145.125 67.55C142.834 67.5533 140.565 67.1045 138.448 66.2294C136.331 65.3542 134.407 64.0699 132.787 62.45C131.168 60.8301 129.883 58.9065 129.008 56.7894C128.133 54.6723 127.684 52.4034 127.688 50.1125C127.688 40.475 135.488 32.675 145.125 32.675Z"
      fill="url(#paint0_linear_518_30)"
    />
    <path
      d="M201.437 399.913C302.987 399.913 296.65 355.875 296.65 355.875L296.525 310.25H199.625V296.563H335.025C335.025 296.563 400 303.938 400 201.475C400 99.0125 343.287 102.65 343.287 102.65H309.437V150.188C309.437 150.188 311.262 206.9 253.625 206.9H157.525C157.525 206.9 103.525 206.025 103.525 259.088V346.825C103.525 346.825 95.3249 399.913 201.437 399.913ZM254.875 369.238C252.584 369.241 250.315 368.792 248.198 367.917C246.081 367.042 244.157 365.757 242.537 364.138C240.917 362.518 239.633 360.594 238.758 358.477C237.883 356.36 237.434 354.091 237.437 351.8C237.437 342.175 245.237 334.375 254.875 334.375C264.512 334.375 272.312 342.163 272.312 351.8C272.312 361.45 264.512 369.238 254.875 369.238Z"
      fill="url(#paint1_linear_518_30)"
    />
    <defs>
      <linearGradient
        id="paint0_linear_518_30"
        x1="38.4375"
        y1="36.775"
        x2="236.225"
        y2="235.225"
        gradientUnits="userSpaceOnUse"
      >
        <stop stop-color="#387EB8" />
        <stop offset="1" stopColor="#366994" />
      </linearGradient>
      <linearGradient
        id="paint1_linear_518_30"
        x1="160.112"
        y1="163.025"
        x2="372.537"
        y2="366.538"
        gradientUnits="userSpaceOnUse"
      >
        <stop stop-color="#FFE052" />
        <stop offset="1" stopColor="#FFC331" />
      </linearGradient>
    </defs>
  </svg>
)
const TsLogo = ({ className }: { className: string }) => (
  <svg
    fill="none"
    height="128"
    viewBox="0 0 128 128"
    width="128"
    xmlns="http://www.w3.org/2000/svg"
    className={className}
  >
    <rect fill="#3178c6" height="128" rx="6" width="128" />
    <path
      clip-rule="evenodd"
      d="m74.2622 99.468v14.026c2.2724 1.168 4.9598 2.045 8.0625 2.629 3.1027.585 6.3728.877 9.8105.877 3.3503 0 6.533-.321 9.5478-.964 3.016-.643 5.659-1.702 7.932-3.178 2.272-1.476 4.071-3.404 5.397-5.786 1.325-2.381 1.988-5.325 1.988-8.8313 0-2.5421-.379-4.7701-1.136-6.6841-.758-1.9139-1.85-3.6159-3.278-5.1062-1.427-1.4902-3.139-2.827-5.134-4.0104-1.996-1.1834-4.246-2.3011-6.752-3.353-1.8352-.7597-3.4812-1.4975-4.9378-2.2134-1.4567-.7159-2.6948-1.4464-3.7144-2.1915-1.0197-.7452-1.8063-1.5341-2.3598-2.3669-.5535-.8327-.8303-1.7751-.8303-2.827 0-.9643.2476-1.8336.7429-2.6079s1.1945-1.4391 2.0976-1.9943c.9031-.5551 2.0101-.9861 3.3211-1.2929 1.311-.3069 2.7676-.4603 4.3699-.4603 1.1658 0 2.3958.0877 3.6928.263 1.296.1753 2.6.4456 3.911.8109 1.311.3652 2.585.8254 3.824 1.3806 1.238.5552 2.381 1.198 3.43 1.9285v-13.1051c-2.127-.8182-4.45-1.4245-6.97-1.819s-5.411-.5917-8.6744-.5917c-3.3211 0-6.4674.3579-9.439 1.0738-2.9715.7159-5.5862 1.8336-7.844 3.353-2.2578 1.5195-4.0422 3.4553-5.3531 5.8075-1.311 2.3522-1.9665 5.1646-1.9665 8.4373 0 4.1785 1.2017 7.7433 3.6052 10.6945 2.4035 2.9513 6.0523 5.4496 10.9466 7.495 1.9228.7889 3.7145 1.5633 5.375 2.323 1.6606.7597 3.0954 1.5486 4.3044 2.3668s2.1628 1.7094 2.8618 2.6736c.7.9643 1.049 2.06 1.049 3.2873 0 .9062-.218 1.7462-.655 2.5202s-1.1 1.446-1.9885 2.016c-.8886.57-1.9956 1.016-3.3212 1.337-1.3255.321-2.8768.482-4.6539.482-3.0299 0-6.0305-.533-9.0021-1.6-2.9715-1.066-5.7245-2.666-8.2591-4.799zm-23.5596-34.9136h18.2974v-11.5544h-51v11.5544h18.2079v51.4456h14.4947z"
      fill="#fff"
      fillRule="evenodd"
    />
  </svg>
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
  const { language, setLanguage } = useContext(CodingLanguage)

  return (
    <div className={cn(className, 'w-full rounded-xl bg-[#131313]')} ref={ref} style={style}>
      <div className="text-content-contrast border-b-content2 mb-2 flex w-full cursor-pointer justify-between rounded-t-xl border-b bg-[#131313] text-center font-mono font-bold">
        {Object.keys(data).map((key) => (
          <div
            key={key}
            onClick={() => setLanguage(key)}
            className={cn('w-full grow py-2 transition hover:grayscale-0', {
              grayscale: key !== language,
              'border-b': key === language,
              'text-content3': key !== language
            })}
          >
            {key} <Logo className="h-8 w-8 p-2" lang={key} />
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
