import { useStore } from '@nanostores/react'

import { Button } from '~/elements/Button'
import { Card, CardBody, CardFooter, CardHeader } from '~/elements/Card'

import { $settings } from '~/features/auth/stores/settings'
import {
  OnboardingStep,
  OnboardingStepDescription,
  OnboardingStepTitle
} from '~/features/onboarding/components/OnboardingStep'
import { docsUrls } from '~/features/onboarding/constants'

export function ExploreDocsStep() {
  const { sdkLanguage } = useStore($settings)

  return (
    <OnboardingStep
      title={
        <>
          <OnboardingStepTitle>Explore our docs</OnboardingStepTitle>
          <OnboardingStepDescription>
            Quick-start guides to get you up and running with RushDB.
          </OnboardingStepDescription>
        </>
      }
      content={
        <div className="overflow col-span-full col-start-1 grid w-full grid-cols-1 gap-3 overflow-auto sm:grid-cols-3">
          <Card className="col-start-1">
            <CardHeader title="Configuring Dashboard" />

            <CardBody>
              <p>
                Walk through the process of registering for RushDB and
                generating an API token necessary for using the RushDB SDK.
              </p>
            </CardBody>

            <CardFooter>
              <Button
                size="small"
                target="_blank"
                rel="noopener noreferrer"
                as="a"
                href={docsUrls.dashboard.configuration}
                variant="secondary"
              >
                Explore docs
              </Button>
            </CardFooter>
          </Card>

          <Card>
            <CardHeader title="RushDB SDK introduction" />

            <CardBody>
              <p>
                This section will guide you through installing the SDK and
                setting up your first SDK instance.
              </p>
            </CardBody>

            <CardFooter>
              <Button
                size="small"
                target="_blank"
                rel="noopener noreferrer"
                as="a"
                href={docsUrls.sdk[sdkLanguage].installation}
                variant="secondary"
              >
                Explore docs
              </Button>
            </CardFooter>
          </Card>

          <Card>
            <CardHeader title="Creating and Retrieving Records with the SDK" />

            <CardBody>
              <p>
                Learn how to use the RushDB SDK to create and retrieve simple
                data records.
              </p>
            </CardBody>

            <CardFooter>
              <Button
                size="small"
                target="_blank"
                rel="noopener noreferrer"
                as="a"
                href={docsUrls.sdk[sdkLanguage].usage}
                variant="secondary"
              >
                Explore docs
              </Button>
            </CardFooter>
          </Card>
        </div>
      }
    />
  )
}
