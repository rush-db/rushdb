import { useStore } from '@nanostores/react'
import { Book } from 'lucide-react'
import { Fragment } from 'react'
import { Button } from '~/elements/Button'
import { Card, CardBody } from '~/elements/Card'
import { Editor } from '~/elements/Editor'
import { FormField, Label } from '~/elements/FormField'
import { CopyInput } from '~/elements/Input'
import { Link } from '~/elements/Link'
import { $settings } from '~/features/auth/stores/settings'
import {
  OnboardingStep,
  OnboardingStepDescription,
  OnboardingStepTitle,
  OnboardingSubStep
} from '~/features/onboarding/components/OnboardingStep'
import { SelectSdkLanguage } from '~/features/onboarding/components/SelectSdkLanguage'
import { docsUrls } from '~/features/onboarding/constants'
import { AvailableSdkLanguage } from '~/features/onboarding/types'
import { $currentProjectFirstToken } from '~/features/projects/stores/current-project'
import { Project } from '~/features/projects/types'
import { getRoutePath } from '~/lib/router'
import { cn, getNumberOfLines } from '~/lib/utils'

const TOKEN_FALLBACK = 'TOKEN'

const getCreateFirstRecordSteps = ({ language }: { language: AvailableSdkLanguage }) => {
  const javascriptSteps = {
    defineModel: {
      title: 'Define your first model',
      code: `import { Model } from '@rushdb/javascript-sdk';

export const UserRepo = new Model(
  'user', 
  {
    name: { type: 'string' },
  },
  db
);
`
    },
    createRecord: {
      title: 'Create your first record',
      code: `await UserRepo.create({
  name: 'John Doe',
});`
    }
  }

  return {
    javascript: javascriptSteps,
    typescript: javascriptSteps
  }[language]
}

const getInstallationCode = ({ token, language }: { token: string; language: AvailableSdkLanguage }) => {
  const jsCode = (token: string) => `import RushDB from '@rushdb/javascript-sdk';

const db = new RushDB("${token}");`

  return {
    javascript: jsCode(token),
    typescript: jsCode(token)
  }[language]
}

export function UseSdkStep({ projectId }: { projectId?: Project['id'] }) {
  const { token, loading } = useStore($currentProjectFirstToken)

  const { sdkLanguage } = useStore($settings)

  return (
    <OnboardingStep
      stickyHeader
      title={
        <>
          <OnboardingStepTitle>Connect through your library of choice</OnboardingStepTitle>

          <OnboardingStepDescription>Interact with RushDB using our SDKs.</OnboardingStepDescription>

          <Button
            size="medium"
            variant="outline"
            as="a"
            href={docsUrls.sdk[sdkLanguage].installation}
            className="sm:mt-5"
          >
            Documentation
            <Book />
          </Button>
        </>
      }
      content={
        <div className="grid grid-cols-[max-content_1fr] gap-3 sm:col-span-2">
          <OnboardingSubStep index={1}>Authorization</OnboardingSubStep>

          <p className="text-content2 col-span-2 sm:col-span-1 sm:col-start-2">
            Both SDK and API require an API Key for every request.{' '}
            {(token || loading) && (
              <span
                className={cn({
                  skeleton: loading
                })}
              >
                We created one for you to get you started as soon as possible.
              </span>
            )}
          </p>

          <CopyInput
            readOnly
            value={token?.value ?? TOKEN_FALLBACK}
            className={cn('col-span-2 sm:col-span-1 sm:col-start-2', {
              skeleton: loading
            })}
          />

          <p className="text-content2 col-span-2 sm:col-span-1 sm:col-start-2">
            You can issue your own API keys from the{' '}
            <Link
              disabled={!projectId}
              href={projectId ? getRoutePath('projectTokens', { id: projectId }) : ''}
            >
              API keys page
            </Link>
            .
          </p>

          <>
            <OnboardingSubStep index={2}>Install SDK</OnboardingSubStep>

            <SelectSdkLanguage className="col-span-2 -ml-1 sm:col-span-1 sm:col-start-2" />

            <InstallSdk className="col-span-2 sm:col-span-1 sm:col-start-2" />
          </>

          <>
            <OnboardingSubStep index={3}>Initialize SDK</OnboardingSubStep>
            <Card className="bg-fill col-span-2 rounded-md p-1 sm:col-span-1 sm:col-start-2">
              <InitializeSdk />
            </Card>
          </>

          <>
            <OnboardingSubStep index={4}>Start building</OnboardingSubStep>
            <StartBuilding className="col-span-2 sm:col-span-1 sm:col-start-2" />
          </>

          <>
            <OnboardingSubStep index={5}>See your records update</OnboardingSubStep>

            <p className="text-content2 col-span-2 sm:col-span-1 sm:col-start-2">
              You can see your records update in the dashboard on the{' '}
              <Link href={projectId ? getRoutePath('project', { id: projectId }) : ''}>Records</Link> page .
            </p>
          </>
        </div>
      }
    />
  )
}

function InstallSdk({ className }: { className?: string }) {
  const { sdkLanguage } = useStore($settings)

  switch (sdkLanguage) {
    case 'typescript':
      return (
        <Card className={className}>
          <CardBody className="pt-5">
            <FormField label="Install core sdk">
              <CopyInput value="npm install @rushdb/javascript-sdk" />
            </FormField>
          </CardBody>
        </Card>
      )
    default:
      return null
  }
}

function InitializeSdk({ className }: { className?: string }) {
  const { sdkLanguage } = useStore($settings)

  const { token, loading } = useStore($currentProjectFirstToken)

  const code = getInstallationCode({
    token: token?.value ?? TOKEN_FALLBACK,
    language: sdkLanguage
  })

  return (
    <Editor
      value={code}
      height={`${getNumberOfLines(code) * 1.2}em`}
      readOnly
      lineNumbers="off"
      className={cn(
        {
          skeleton: loading
        },
        className
      )}
    />
  )
}

function StartBuilding({ className }: { className?: string }) {
  const { sdkLanguage } = useStore($settings)

  const steps = getCreateFirstRecordSteps({
    language: sdkLanguage
  })

  return (
    <>
      {Object.values(steps).map(({ title, code }) => (
        <Fragment key={title}>
          <Label className={className}>{title}</Label>

          <Card className={className}>
            <Editor
              defaultValue={code}
              height={`${getNumberOfLines(code) * 1.2}em`}
              readOnly
              className={className}
              lineNumbers="off"
            />
          </Card>
        </Fragment>
      ))}
    </>
  )
}
