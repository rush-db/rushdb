import {
  AVAILABLE_SDK_LANGUAGES,
  SDK_LANGUAGES
} from '~/features/onboarding/constants'

export type SdkLanguage = (typeof SDK_LANGUAGES)[number]

export type AvailableSdkLanguage = Extract<
  SdkLanguage,
  (typeof AVAILABLE_SDK_LANGUAGES)[number]
>

export const isSdkLanguageAvailable = (
  language: unknown
): language is AvailableSdkLanguage => {
  return AVAILABLE_SDK_LANGUAGES.includes(language as AvailableSdkLanguage)
}
