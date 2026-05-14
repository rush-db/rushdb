import type { SdkLanguage } from '~/features/onboarding/types'

import jsLogo from './assets/js-logo.png'
import pythonLogo from './assets/python-logo.png'
import typescriptLogo from './assets/ts-logo.png'

export const docsUrls = {
  sdk: {
    typescript: {
      installation: 'https://docs.rushdb.com/typescript-sdk/introduction',
      usage: 'https://docs.rushdb.com/typescript-sdk/records/create-records',
      github: 'https://github.com/rush-db/rushdb',
      logo: typescriptLogo
    },
    python: {
      installation: 'https://docs.rushdb.com/python-sdk/introduction',
      usage: 'https://docs.rushdb.com/python-sdk/records/create-records',
      github: 'https://github.com/rush-db/rushdb-python',
      logo: pythonLogo
    },
    shell: {
      installation: 'https://docs.rushdb.com/rest-api/introduction',
      usage: 'https://docs.rushdb.com/rest-api/records/import-data',
      github: 'https://github.com/rush-db/rushdb',
      logo: jsLogo
    }
  },
  dashboard: {
    configuration: 'https://docs.rushdb.com/get-started/quick-tutorial'
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

export const SDK_LANGUAGES = ['python', 'typescript', 'shell'] as const

export const AVAILABLE_SDK_LANGUAGES = [
  'python',
  'typescript',
  'shell'
] as const satisfies readonly SdkLanguage[]
