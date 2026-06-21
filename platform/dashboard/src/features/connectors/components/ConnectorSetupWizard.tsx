import { Cable, Check, Database, ListFilter, LockKeyhole, Server } from 'lucide-react'
import { useMemo, useState } from 'react'

import { Button } from '~/elements/Button'
import { TextField } from '~/elements/Input'
import { useCreateConnectorMutation } from '~/features/connectors/hooks'
import type { ConnectorType } from '~/features/connectors/types'
import { cn } from '~/lib/utils'

type Step = 'connection' | 'scope' | 'rules' | 'review'

const steps: Array<{ id: Step; label: string; icon: typeof Server }> = [
  { id: 'connection', label: 'Connection', icon: Server },
  { id: 'scope', label: 'Scope', icon: Database },
  { id: 'rules', label: 'Rules', icon: ListFilter },
  { id: 'review', label: 'Review', icon: Check }
]

export function ConnectorSetupWizard({
  onClose,
  sourceType
}: {
  onClose: () => void
  sourceType: ConnectorType
}) {
  const { mutateAsync: createConnector, isPending } = useCreateConnectorMutation()
  const [step, setStep] = useState<Step>('connection')
  const [name, setName] = useState(sourceType === 'postgres' ? 'PostgreSQL source' : 'MongoDB source')
  const [host, setHost] = useState('')
  const [port, setPort] = useState(sourceType === 'postgres' ? '5432' : '27017')
  const [database, setDatabase] = useState('')
  const [user, setUser] = useState('')
  const [password, setPassword] = useState('')
  const [entities, setEntities] = useState('')
  const [ignore, setIgnore] = useState('')
  const [error, setError] = useState<string | null>(null)

  const stepIndex = steps.findIndex((item) => item.id === step)
  const selectedEntities = useMemo(
    () =>
      entities
        .split(',')
        .map((value) => value.trim())
        .filter(Boolean),
    [entities]
  )
  const ignorePatterns = useMemo(
    () =>
      ignore
        .split('\n')
        .map((value) => value.trim())
        .filter(Boolean),
    [ignore]
  )

  const canContinue = step !== 'connection' || Boolean(host && database)

  const submit = async () => {
    setError(null)
    try {
      await createConnector({
        name,
        type: sourceType,
        config:
          sourceType === 'postgres' ?
            {
              host,
              port: Number(port || 5432),
              database,
              user,
              tables: selectedEntities,
              snapshot: true
            }
          : {
              host,
              port: Number(port || 27017),
              database,
              user,
              collections: selectedEntities,
              snapshot: true
            },
        secrets: { password },
        transform: {
          naming: 'preserve',
          mergeStrategy: 'append',
          fields: {
            ignore: ignorePatterns
          }
        }
      })
      onClose()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Could not create connector')
    }
  }

  const goNext = () => {
    if (stepIndex < steps.length - 1) {
      setStep(steps[stepIndex + 1].id)
    }
  }

  const goBack = () => {
    if (stepIndex > 0) {
      setStep(steps[stepIndex - 1].id)
    }
  }

  return (
    <div className="border-border bg-fill2 mt-5 rounded-lg border">
      <div className="flex items-start justify-between border-b px-5 py-4">
        <div>
          <h3 className="flex items-center gap-2 text-base font-semibold">
            <Cable size={18} />
            Connect {sourceType === 'postgres' ? 'PostgreSQL' : 'MongoDB'}
          </h3>
          <p className="text-content2 text-sm">Configure a paused connector, then test and resume it.</p>
        </div>
        <Button onClick={onClose} size="xsmall" variant="ghost">
          Close
        </Button>
      </div>

      <div className="grid grid-cols-1 gap-5 p-5 lg:grid-cols-[220px_1fr]">
        <ol className="space-y-2">
          {steps.map(({ id, label, icon: Icon }, index) => (
            <li
              className={cn(
                'flex items-center gap-2 rounded-md px-3 py-2 text-sm',
                step === id ? 'bg-secondary text-content' : 'text-content2'
              )}
              key={id}
            >
              <Icon size={15} />
              <span>
                {index + 1}. {label}
              </span>
            </li>
          ))}
        </ol>

        <div className="min-h-[260px]">
          {step === 'connection' && (
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              <TextField
                label="Name"
                size="small"
                value={name}
                onChange={(event) => setName(event.target.value)}
              />
              <TextField
                label="Host"
                size="small"
                value={host}
                onChange={(event) => setHost(event.target.value)}
              />
              <TextField
                label="Port"
                size="small"
                value={port}
                onChange={(event) => setPort(event.target.value)}
              />
              <TextField
                label="Database"
                size="small"
                value={database}
                onChange={(event) => setDatabase(event.target.value)}
              />
              <TextField
                label="User"
                size="small"
                value={user}
                onChange={(event) => setUser(event.target.value)}
              />
              <TextField
                label="Password"
                size="small"
                type="password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
              />
            </div>
          )}

          {step === 'scope' && (
            <TextField
              label={sourceType === 'postgres' ? 'Tables' : 'Collections'}
              caption="Comma-separated. Leave empty to configure later."
              size="small"
              value={entities}
              onChange={(event) => setEntities(event.target.value)}
            />
          )}

          {step === 'rules' && (
            <label className="flex flex-col gap-1 text-sm">
              <span className="font-medium">Ignore patterns</span>
              <textarea
                className="scrollbar-thin bg-secondary min-h-[190px] resize-none rounded-md border p-3 font-mono text-sm outline-none focus-visible:ring"
                placeholder={'email\npayment.**\n*.token'}
                value={ignore}
                onChange={(event) => setIgnore(event.target.value)}
              />
            </label>
          )}

          {step === 'review' && (
            <div className="grid gap-3 text-sm">
              <div className="rounded-md border p-3">
                <p className="font-semibold">{name}</p>
                <p className="text-content2">
                  {sourceType} · {host}:{port} · {database}
                </p>
              </div>
              <div className="rounded-md border p-3">
                <p className="font-semibold">Scope</p>
                <p className="text-content2">
                  {selectedEntities.length ? selectedEntities.join(', ') : 'Configure later'}
                </p>
              </div>
              <div className="rounded-md border p-3">
                <p className="font-semibold">Ignored fields</p>
                <p className="text-content2">{ignorePatterns.length ? ignorePatterns.join(', ') : 'None'}</p>
              </div>
              <p className="text-content2 flex items-center gap-2">
                <LockKeyhole size={14} />
                Secrets are write-only and will not be shown after saving.
              </p>
            </div>
          )}

          {error && <p className="text-danger mt-3 text-sm">{error}</p>}
        </div>
      </div>

      <div className="flex justify-between border-t px-5 py-4">
        <Button disabled={stepIndex === 0 || isPending} onClick={goBack} size="small" variant="outline">
          Back
        </Button>
        {step === 'review' ?
          <Button disabled={isPending} loading={isPending} onClick={submit} size="small" variant="primary">
            Create connector
          </Button>
        : <Button disabled={!canContinue} onClick={goNext} size="small" variant="primary">
            Continue
          </Button>
        }
      </div>
    </div>
  )
}
