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
import type { AvailableSdkLanguage } from '~/features/onboarding/types'
import { useProjectTokensQuery } from '~/features/projects/hooks/useProjectQueries'
import type { Project } from '~/features/projects/types'
import { getRoutePath } from '~/lib/router'
import { cn, getNumberOfLines } from '~/lib/utils'
import { setTourStep } from '~/features/tour/stores/tour.ts'

const TOKEN_FALLBACK = 'TOKEN'

const getCreateFirstRecordSteps = ({
  language,
  token
}: {
  language: AvailableSdkLanguage
  token: string
}) => {
  const typescriptSteps = {
    pushAndQueryDataTs: {
      title: 'Push and query data',
      code: `await db.records.importJson({ label: 'COMPANY', data: jsonData })

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

  const shellSteps = {
    pushAndQueryDataShell: {
      title: 'Push and query data',
      code: `curl -X POST 'https://api.rushdb.com/api/v1/records/import/json' \\
  -H 'accept: */*' \\
  -H 'Authorization: Bearer ${token}' \\
  -H 'Content-Type: application/json' \\
  -d '{
    "label": "COMPANY",
    "data": [
      {
        "name": "OpenAI",
        "rating": 5,
        "category": "AI"
      }
    ]
  }'

curl -X POST 'https://api.rushdb.com/api/v1/records/search' \\
  -H 'accept: */*' \\
  -H 'Authorization: Bearer ${token}' \\
  -H 'Content-Type: application/json' \\
  -d '{
    "labels": ["COMPANY"],
    "where": {
      "rating": { "$gte": 4 },
      "name": { "$contains": "AI" }
    }
  }'`
    }
  }

  return {
    typescript: typescriptSteps,
    python: pythonSteps,
    shell: shellSteps
  }[language]
}

const getInstallationCode = ({ token, language }: { token: string; language: AvailableSdkLanguage }) => {
  const jsCode = (token: string) => `import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('${token}')`

  const pyCode = (token: string) => `from rushdb import RushDB

db = RushDB("${token}")`

  const shellCode = (token: string) => `curl -X POST 'https://api.rushdb.com/api/v1/records/search' \\
  -H 'Authorization: Bearer ${token}' \\
  -H 'Content-Type: application/json' \\
  -d '{}'`

  return {
    typescript: jsCode(token),
    python: pyCode(token),
    shell: shellCode(token)
  }[language]
}

export function UseSdkStep({ projectId }: { projectId?: Project['id'] }) {
  const { data: tokens, isPending: loading } = useProjectTokensQuery()
  const token = tokens?.[0]

  useEffect(() => {
    if (!loading && token?.value) {
      setTourStep('projectSdkTokenOverview', false)
    }
  }, [loading, token])

  const { sdkLanguage } = useStore($settings)
  const showInstallStep = sdkLanguage !== 'shell'

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
          <OnboardingSubStep index={1}>Copy authorization token</OnboardingSubStep>

          <CopyInput
            data-tour="project-help-sdk-input"
            readOnly
            value={token?.value ?? TOKEN_FALLBACK}
            className={cn('col-span-2 sm:col-span-1 sm:col-start-2', {
              skeleton: loading
            })}
          />

          <OnboardingSubStep index={2}>Setup</OnboardingSubStep>

          <div className="col-span-2 flex flex-col gap-2 sm:col-span-1 sm:col-start-2">
            <SelectSdkLanguage className="-ml-1" />
            {showInstallStep && <InstallSdk />}
            <Card className="bg-fill rounded-md p-1">
              <InitializeSdk />
            </Card>
          </div>

          <OnboardingSubStep index={3}>Push data</OnboardingSubStep>

          <StartBuilding className="col-span-2 sm:col-span-1 sm:col-start-2" />

          <OnboardingSubStep index={4}>See your records update</OnboardingSubStep>

          <p className="text-content2 col-span-2 sm:col-span-1 sm:col-start-2">
            Head to the{' '}
            <Link href={projectId ? getRoutePath('project', { id: projectId }) : ''}>Records</Link> page to
            see your data live.
          </p>
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

  const { data: tokens, isPending: loading } = useProjectTokensQuery()
  const token = tokens?.[0]

  const code = getInstallationCode({
    token: token?.value ?? TOKEN_FALLBACK,
    language: sdkLanguage
  })

  return (
    <Editor
      value={code}
      height={`${getNumberOfLines(code) * 1.2}em`}
      defaultLanguage={sdkLanguage === 'shell' ? 'shell' : sdkLanguage}
      format={sdkLanguage !== 'shell'}
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
  const { data: tokens } = useProjectTokensQuery()
  const token = tokens?.[0]

  const [editorVisible, setEditorVisible] = useState<boolean>(true)

  const steps = useMemo(
    () =>
      getCreateFirstRecordSteps({
        language: sdkLanguage,
        token: token?.value ?? TOKEN_FALLBACK
      }),
    [sdkLanguage, token?.value]
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
                defaultLanguage={sdkLanguage === 'shell' ? 'shell' : sdkLanguage}
                format={sdkLanguage !== 'shell'}
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
