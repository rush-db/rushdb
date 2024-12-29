import { useStore } from '@nanostores/react'
import { Badge } from '~/elements/Badge'
import { Tab, Tabs, TabsList } from '~/elements/Tabs'
import { $settings } from '~/features/auth/stores/settings'
import { SDK_LANGUAGES } from '~/features/onboarding/constants'
import {
  SdkLanguage,
  isSdkLanguageAvailable
} from '~/features/onboarding/types'
import { capitalize, cn } from '~/lib/utils'

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
        if (isSdkLanguageAvailable(value))
          $settings.setKey('sdkLanguage', value)
      }}
      className={cn('w-full overflow-auto sm:w-auto', className)}
    >
      <TabsList>
        {languages.map((language) => {
          const available = isSdkLanguageAvailable(language)

          return (
            <Tab value={language} disabled={!available} key={language}>
              {capitalize(language)}

              {!available && <Badge>Soon</Badge>}
            </Tab>
          )
        })}
      </TabsList>
    </Tabs>
  )
}
