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
import { isSdkLanguageAvailable } from '~/features/onboarding/types'
import { capitalize } from '~/lib/utils'

export function ClientLibrariesStep() {
  return (
    <OnboardingStep
      stickyHeader
      title={
        <div className="flex flex-col gap-3">
          <OnboardingStepTitle className="col-span-1">
            Client libraries
          </OnboardingStepTitle>
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
              <div
                className="flex items-center gap-5 sm:flex-col sm:items-start"
                key={language}
              >
                <img
                  src={docsUrls.sdk[language].logo}
                  className="h-[60px] w-[60px] shrink-0 sm:h-20 sm:w-20"
                />

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
