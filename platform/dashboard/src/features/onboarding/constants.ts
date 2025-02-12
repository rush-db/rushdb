import type { SdkLanguage } from '~/features/onboarding/types'

import pythonLogo from './assets/python-logo.png'
import rubyLogo from './assets/ruby-logo.png'
import typescriptLogo from './assets/ts-logo.png'

export const docsUrls = {
  sdk: {
    typescript: {
      installation: 'https://docs.rushdb.com/quick-start/installation/',
      usage: 'https://docs.rushdb.com/quick-start/creating-and-retrieving-records',
      github: 'https://github.com/rush-db/rushdb',
      logo: typescriptLogo
    },
    python: {
      installation: 'https://docs.rushdb.com/quick-start/installation/',
      usage: 'https://docs.rushdb.com/python-sdk/records-api',
      github: 'https://github.com/rush-db/rushdb-python',
      logo: pythonLogo
    }
  },
  dashboard: {
    configuration: 'https://docs.rushdb.com/quick-start/configuring-dashboard'
  }
} satisfies {
  dashboard: {
    configuration: string
  }
  sdk: Record<
    SdkLanguage,
    {
      github: string | undefined
      installation: string | undefined
      logo: string
      usage: string | undefined
    }
  >
}

export const SDK_LANGUAGES = ['typescript', 'python'] as const

export const AVAILABLE_SDK_LANGUAGES = ['typescript', 'python'] as const satisfies readonly SdkLanguage[]
