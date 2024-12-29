import { SdkLanguage } from '~/features/onboarding/types'

import javascriptLogo from './assets/js-logo.png'
import typescriptLogo from './assets/ts-logo.png'
import pythonLogo from './assets/python-logo.png'
import rubyLogo from './assets/ruby-logo.png'

export const docsUrls = {
  sdk: {
    typescript: {
      installation: 'https://docs.collect.so/quick-start/installation/',
      usage:
        'https://docs.collect.so/quick-start/creating-and-retrieving-records',
      github: 'https://github.com/collect-so/collect',
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
    configuration: 'https://docs.collect.so/quick-start/configuring-dashboard'
  }
} satisfies {
  sdk: Record<
    SdkLanguage,
    {
      installation: undefined | string
      usage: undefined | string
      github: undefined | string
      logo: string
    }
  >
  dashboard: {
    configuration: string
  }
}

export const SDK_LANGUAGES = ['typescript', 'python', 'ruby'] as const

export const AVAILABLE_SDK_LANGUAGES = [
  'typescript'
] as const satisfies readonly SdkLanguage[]
