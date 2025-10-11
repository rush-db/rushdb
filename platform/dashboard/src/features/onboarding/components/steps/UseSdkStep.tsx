import { useStore } from '@nanostores/react'
import { Book } from 'lucide-react'
import { Fragment, useEffect, useMemo, useState } from 'react'
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
import { setTourStep } from '~/features/tour/stores/tour.ts'

const TOKEN_FALLBACK = 'TOKEN'

const getCreateFirstRecordSteps = ({ language }: { language: AvailableSdkLanguage }) => {
  const typescriptSteps = {
    pushAndQueryDataTs: {
      title: 'Push and query data',
      code: `await db.records.createMany({ label: 'COMPANY', data: jsonData })

await db.records.find({
  labels: ['COMPANY'],
  where: {
    rating: { $gte: 4 },
    name: { $contains: 'AI' }
  }
})`
    }
  }

  const pythonSteps = {
    pushAndQueryDataPy: {
      title: 'Push and query data',
      code: `db.records.create_many("COMPANY", jsonData)
      
db.records.find({
    "labels": ["COMPANY"],
    "where": {
      "rating": { "$gte": 4 },
      "name": { "$contains": "AI" }
    }
})`
    }
  }

  return {
    typescript: typescriptSteps,
    python: pythonSteps
  }[language]
}

const getInstallationCode = ({ token, language }: { token: string; language: AvailableSdkLanguage }) => {
  const jsCode = (token: string) => `import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('${token}')`

  const pyCode = (token: string) => `from rushdb import RushDB

db = RushDB("${token}")`

  return {
    typescript: jsCode(token),
    python: pyCode(token)
  }[language]
}

export function UseSdkStep({ projectId }: { projectId?: Project['id'] }) {
  const { token, loading } = useStore($currentProjectFirstToken)

  useEffect(() => {
    if (!loading && token?.value) {
      setTourStep('projectSdkTokenOverview', false)
    }
  }, [loading, token])

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
            data-tour="project-help-sdk-input"
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
    case 'python': {
      return (
        <Card className={className}>
          <CardBody className="pt-5">
            <FormField label="Install core sdk">
              <CopyInput value="pip install rushdb" />
            </FormField>
          </CardBody>
        </Card>
      )
    }
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

  const [editorVisible, setEditorVisible] = useState<boolean>(true)

  const steps = useMemo(
    () =>
      getCreateFirstRecordSteps({
        language: sdkLanguage
      }),
    [sdkLanguage]
  )

  // Dirty hack to reset Editor when language has changed
  useEffect(() => {
    setEditorVisible(false)
    const timeout = setTimeout(() => {
      setEditorVisible(true)
    }, 0)
    return () => {
      clearTimeout(timeout)
    }
  }, [sdkLanguage])

  return (
    <>
      {Object.values(steps).map(({ title, code }) => (
        <Fragment key={title}>
          <Label className={className}>{title}</Label>

          <Card className={className}>
            {editorVisible && (
              <Editor
                defaultValue={code}
                height={`${getNumberOfLines(code) * 1.2}em`}
                readOnly
                className={className}
                lineNumbers="off"
              />
            )}
          </Card>
        </Fragment>
      ))}
    </>
  )
}
