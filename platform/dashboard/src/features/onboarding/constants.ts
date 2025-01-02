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
      installation: undefined,
      usage: undefined,
      github: undefined,
      logo: pythonLogo
    },
    ruby: {
      installation: undefined,
      usage: undefined,
      github: undefined,
      logo: rubyLogo
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

export const SDK_LANGUAGES = ['typescript', 'python', 'ruby'] as const

export const AVAILABLE_SDK_LANGUAGES = ['typescript'] as const satisfies readonly SdkLanguage[]
