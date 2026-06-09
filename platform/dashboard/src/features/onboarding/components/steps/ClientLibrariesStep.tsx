import { Book, Github } from 'lucide-react'
import { Badge } from '~/elements/Badge'
import { Button } from '~/elements/Button'
import { ButtonGroup } from '~/elements/ButtonGroup'
import {
  OnboardingStep,
  OnboardingStepDescription,
  OnboardingStepTitle
} from '~/features/onboarding/components/OnboardingStep'
import { SDK_LANGUAGES, docsUrls } from '~/features/onboarding/constants'
import type { SdkLanguage } from '~/features/onboarding/types'
import { isSdkLanguageAvailable } from '~/features/onboarding/types'
import { capitalize } from '~/lib/utils'

function ShellLogo({ className }: { className?: string }) {
  return (
    <div className={`flex items-center justify-center rounded-xl bg-[#1e1e1e] ${className ?? ''}`}>
      <svg
        xmlns="http://www.w3.org/2000/svg"
        viewBox="0 0 24 24"
        width="50%"
        height="50%"
        fill="none"
        stroke="white"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
      >
        <polyline points="4 17 10 11 4 5" />
        <line x1="12" y1="19" x2="20" y2="19" />
      </svg>
    </div>
  )
}

function LanguageLogo({ language, className }: { language: SdkLanguage; className?: string }) {
  if (language === 'shell') {
    return <ShellLogo className={className} />
  }
  return <img src={docsUrls.sdk[language].logo} className={className} />
}

export function ClientLibrariesStep() {
  return (
    <OnboardingStep
      stickyHeader
      title={
        <div className="flex flex-col gap-3">
          <OnboardingStepTitle className="col-span-1">Client libraries</OnboardingStepTitle>
          <OnboardingStepDescription className="col-span-1 col-start-1">
            Choose the language you want to use to interact with RushDB.
          </OnboardingStepDescription>
        </div>
      }
      content={
        <div className="col-span-2 grid grid-cols-1 gap-5 sm:grid-cols-2 sm:gap-10">
          {SDK_LANGUAGES.map((language) => {
            const available = isSdkLanguageAvailable(language)

            return (
              <div className="flex items-center gap-5 sm:flex-col sm:items-start" key={language}>
                <LanguageLogo language={language} className="h-[60px] w-[60px] shrink-0 sm:h-20 sm:w-20" />

                <div className="flex flex-col gap-2">
                  <h4 className="flex items-center gap-1">
                    {capitalize(language)}
                    {available ? '' : <Badge>Coming soon</Badge>}
                  </h4>

                  <ButtonGroup>
                    <Button
                      as="a"
                      href={docsUrls.sdk[language].installation}
                      size="xsmall"
                      target="_blank"
                      rel="noopener noreferrer"
                      variant="outline"
                      disabled={!available}
                    >
                      Docs
                      <Book />
                    </Button>
                    <Button
                      as="a"
                      href={docsUrls.sdk[language].github}
                      size="xsmall"
                      target="_blank"
                      rel="noopener noreferrer"
                      variant="outline"
                      disabled={!available}
                    >
                      View on GitHub
                      <Github />
                    </Button>
                  </ButtonGroup>
                </div>
              </div>
            )
          })}
        </div>
      }
    />
  )
}
