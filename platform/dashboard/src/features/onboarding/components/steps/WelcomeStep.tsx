import { PageTitle } from '~/elements/PageHeader'
import { Skeleton } from '~/elements/Skeleton'

import {
  OnboardingStep,
  OnboardingStepDescription,
  OnboardingStepTitle
} from '~/features/onboarding/components/OnboardingStep'

import { useFilteredRecordsQuery } from '~/features/projects/hooks/useProjectQueries'
import { isProjectEmpty } from '~/features/projects/utils'

export function WelcomeStep({ loading }: { loading: boolean }) {
  const { data: recordsResult, isPending: recordsLoading } = useFilteredRecordsQuery()
  const isEmpty = isProjectEmpty({ totalRecords: recordsResult?.total, loading: recordsLoading })

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
