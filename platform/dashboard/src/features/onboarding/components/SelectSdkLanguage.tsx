import { useStore } from '@nanostores/react'
import { Badge } from '~/elements/Badge'
import { Tab, Tabs, TabsList } from '~/elements/Tabs'
import { $settings } from '~/features/auth/stores/settings'
import { SDK_LANGUAGES } from '~/features/onboarding/constants'
import type { SdkLanguage } from '~/features/onboarding/types'
import { isSdkLanguageAvailable } from '~/features/onboarding/types'
import { cn } from '~/lib/utils'

function PythonIcon() {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 48 48"
      width="16"
      height="16"
      aria-hidden="true"
      style={{ flexShrink: 0 }}
    >
      <path
        fill="#3776AB"
        d="M24.046 5C13.613 5 14.25 9.707 14.25 9.707l.012 4.857h9.965v1.457H11.024S5 14.317 5 24.815c0 10.499 5.81 10.127 5.81 10.127h3.467v-4.873s-.187-5.81 5.713-5.81h9.845s5.529.09 5.529-5.345V11.015S36.162 5 24.046 5zm-5.486 3.174c.993 0 1.8.807 1.8 1.8s-.807 1.8-1.8 1.8-1.8-.807-1.8-1.8.807-1.8 1.8-1.8z"
      />
      <path
        fill="#FFD43B"
        d="M24.24 43c10.433 0 9.796-4.707 9.796-4.707l-.012-4.857h-9.965v-1.457H37.26S43.286 33.683 43.286 23.185c0-10.499-5.81-10.127-5.81-10.127h-3.467v4.873s.187 5.81-5.713 5.81h-9.845s-5.529-.09-5.529 5.345v8.918S12.124 43 24.24 43zm5.486-3.174c-.993 0-1.8-.807-1.8-1.8s.807-1.8 1.8-1.8 1.8.807 1.8 1.8-.807 1.8-1.8 1.8z"
      />
    </svg>
  )
}

function TypeScriptIcon() {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      width="16"
      height="16"
      aria-hidden="true"
      style={{ flexShrink: 0 }}
    >
      <rect fill="#3178C6" width="22" height="22" x="1" y="1" rx="3.5" />
      <text
        x="12"
        y="17"
        textAnchor="middle"
        fill="white"
        fontFamily="'Segoe UI','Helvetica Neue',Arial,sans-serif"
        fontSize="11"
        fontWeight="700"
        letterSpacing="-0.5"
      >
        TS
      </text>
    </svg>
  )
}

function ShellIcon() {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      width="16"
      height="16"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      style={{ flexShrink: 0 }}
    >
      <polyline points="4 17 10 11 4 5" />
      <line x1="12" y1="19" x2="20" y2="19" />
    </svg>
  )
}

const LANG_CONFIG: Record<SdkLanguage, { label: string; Icon: () => React.ReactElement }> = {
  python: { label: 'Python', Icon: PythonIcon },
  typescript: { label: 'TypeScript', Icon: TypeScriptIcon },
  shell: { label: 'Shell', Icon: ShellIcon }
}

export function SelectSdkLanguage({
  className,
  languages = SDK_LANGUAGES
}: {
  className?: string
  languages?: readonly SdkLanguage[]
}) {
  const { sdkLanguage } = useStore($settings)

  return (
    <Tabs
      value={sdkLanguage}
      onValueChange={(value) => {
        if (isSdkLanguageAvailable(value)) $settings.setKey('sdkLanguage', value)
      }}
      className={cn('w-full overflow-auto sm:w-auto', className)}
    >
      <TabsList>
        {languages.map((language) => {
          const available = isSdkLanguageAvailable(language)
          const { label, Icon } = LANG_CONFIG[language]

          return (
            <Tab value={language} disabled={!available} key={language}>
              <span className="flex items-center gap-1.5">
                <Icon />
                {label}
              </span>
              {!available && <Badge>Soon</Badge>}
            </Tab>
          )
        })}
      </TabsList>
    </Tabs>
  )
}
