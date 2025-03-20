import { useStore } from '@nanostores/react'

import { PageTitle } from '~/elements/PageHeader'
import { Skeleton } from '~/elements/Skeleton'

import {
  OnboardingStep,
  OnboardingStepDescription,
  OnboardingStepTitle
} from '~/features/onboarding/components/OnboardingStep'

import { $currentProjectIsEmpty } from '~/features/projects/stores/current-project'

export function WelcomeStep({ loading }: { loading: boolean }) {
  const isEmpty = useStore($currentProjectIsEmpty)

  return (
    <OnboardingStep
      title={
        <>
          <OnboardingStepTitle>
            <Skeleton enabled={loading}>
              <PageTitle> {isEmpty ? 'Welcome to your new project' : 'Help'}</PageTitle>
            </Skeleton>
          </OnboardingStepTitle>

          <OnboardingStepDescription>
            {isEmpty ?
              <>Your project is currently empty. Use steps below to get started.</>
            : <>Use steps below to get started.</>}
          </OnboardingStepDescription>
        </>
      }
    />
  )
}
