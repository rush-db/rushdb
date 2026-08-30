import { useState } from 'react'
import { useStore } from '@nanostores/react'
import { Check, FileUp, Plus, Trash2, UploadCloud, X } from 'lucide-react'

import { Button } from '~/elements/Button'
import { TextField } from '~/elements/Input'
import { Card, CardBody, CardHeader } from '~/elements/Card'
import { SelectField } from '~/elements/Select'
import { cn } from '~/lib/utils'
import { rushDBInstance } from '~/lib/sdk'
import { $currentProjectId } from '~/features/projects/stores/id'
import { $router, getRoutePath } from '~/lib/router'

import {
  detectFormat,
  formatBytes,
  generateClientFileId,
  isSupportedFileName,
  readHeaderColumns,
  suggestLabel
} from '../lib'
import type { ImportFileFormat, ImportFileManifest } from '../types'

const ACCEPT = '.csv,.jsonl,.ndjson,.json,.parquet'

type EndpointDraft = {
  column: string
  label: string
  keyProperty: string
}

type FileDraft = {
  clientFileId: string
  file: File
  fileName: string
  format: ImportFileFormat
  size: number
  role: 'records' | 'links'
  rootLabel: string
  headers: string[]
  headersLoaded: boolean
  relationshipType: string
  source: EndpointDraft
  target: EndpointDraft
  propertyColumns: Record<string, string>
}

type UploadState = {
  phase: 'idle' | 'uploading' | 'complete' | 'error'
  progress: number
}

function makeDraft(file: File): FileDraft {
  const format = detectFormat(file.name) ?? 'csv'
  const label = suggestLabel(file.name)
  return {
    clientFileId: generateClientFileId(),
    file,
    fileName: file.name,
    format,
    size: file.size,
    role: 'records',
    rootLabel: label,
    headers: [],
    headersLoaded: false,
    relationshipType: 'RELATED_TO',
    source: { column: '', label, keyProperty: 'id' },
    target: { column: '', label, keyProperty: 'id' },
    propertyColumns: {}
  }
}

function isRecordFileValid(draft: FileDraft): boolean {
  return Boolean(draft.rootLabel.trim())
}

function isLinksFileValid(draft: FileDraft): boolean {
  return Boolean(
    draft.relationshipType.trim() &&
      draft.source.column.trim() &&
      draft.source.label.trim() &&
      draft.source.keyProperty.trim() &&
      draft.target.column.trim() &&
      draft.target.label.trim() &&
      draft.target.keyProperty.trim()
  )
}

function isFileValid(draft: FileDraft): boolean {
  return draft.role === 'records' ? isRecordFileValid(draft) : isLinksFileValid(draft)
}

function RoleToggle({
  role,
  onChange
}: {
  role: 'records' | 'links'
  onChange: (role: 'records' | 'links') => void
}) {
  const options: Array<{ value: 'records' | 'links'; label: string }> = [
    { value: 'records', label: 'Data' },
    { value: 'links', label: 'Relationships' }
  ]
  return (
    <div className="flex items-center gap-1 rounded-md border bg-secondary p-1">
      {options.map((opt) => {
        const active = role === opt.value
        return (
          <button
            key={opt.value}
            className={cn(
              'h-7 rounded-sm px-3 text-xs font-medium transition',
              active ? 'bg-fill text-content shadow-sm' : 'text-content2 hover:text-content'
            )}
            onClick={() => onChange(opt.value)}
            type="button"
          >
            {opt.label}
          </button>
        )
      })}
    </div>
  )
}

function EndpointEditor({
  title,
  endpoint,
  headers,
  onChange
}: {
  title: string
  endpoint: EndpointDraft
  headers: string[]
  onChange: (patch: Partial<EndpointDraft>) => void
}) {
  const headerOptions = headers.map((h) => ({ value: h, label: h }))
  return (
    <div className="flex flex-col gap-3 rounded-md border p-3">
      <p className="text-sm font-semibold">{title}</p>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
        {headers.length ?
          <SelectField
            label="Column"
            size="small"
            value={endpoint.column}
            onChange={(e) => onChange({ column: e.target.value })}
            options={headerOptions}
          />
        : <TextField
            label="Column"
            size="small"
            placeholder="Column name"
            value={endpoint.column}
            onChange={(e) => onChange({ column: e.target.value })}
          />
        }
        <TextField
          label="Label"
          size="small"
          value={endpoint.label}
          onChange={(e) => onChange({ label: e.target.value })}
        />
        <TextField
          label="Key property"
          size="small"
          value={endpoint.keyProperty}
          onChange={(e) => onChange({ keyProperty: e.target.value })}
        />
      </div>
    </div>
  )
}

function FileEditor({
  draft,
  onRemove,
  onRoleChange,
  onRootLabel,
  onRelationshipType,
  onEndpoint,
  onPropertyColumn
}: {
  draft: FileDraft
  onRemove: () => void
  onRoleChange: (role: 'records' | 'links') => void
  onRootLabel: (value: string) => void
  onRelationshipType: (value: string) => void
  onEndpoint: (endpoint: 'source' | 'target', patch: Partial<EndpointDraft>) => void
  onPropertyColumn: (column: string, property: string) => void
}) {
  const valid = isFileValid(draft)

  return (
    <Card>
      <CardHeader
        className="flex items-center gap-3 pb-3"
        title={
          <div className="flex min-w-0 items-center gap-2">
            <FileUp size={18} className="shrink-0 text-content2" />
            <span className="min-w-0 truncate">{draft.fileName}</span>
            <span className="font-mono text-xs font-normal text-content3">
              {draft.format} · {formatBytes(draft.size)}
            </span>
            <span
              className={cn('ml-1', valid ? 'text-success' : 'text-content3')}
              title={valid ? 'Valid' : 'Missing required fields'}
            >
              {valid ?
                <Check size={14} />
              : <X size={14} />}
            </span>
          </div>
        }
      >
        <div className="mt-3 flex items-center justify-between gap-3">
          <RoleToggle role={draft.role} onChange={onRoleChange} />
          <Button onClick={onRemove} size="xsmall" variant="ghost" aria-label="Remove file">
            <Trash2 size={14} />
          </Button>
        </div>
      </CardHeader>

      <CardBody className="pt-0">
        {draft.role === 'records' ?
          <div className="max-w-md">
            <TextField
              label="Root label"
              size="small"
              caption="Label applied to every record parsed from this file."
              value={draft.rootLabel}
              onChange={(e) => onRootLabel(e.target.value)}
            />
          </div>
        : <div className="flex flex-col gap-3">
            <div className="grid grid-cols-1 gap-3 sm:max-w-md">
              <TextField
                label="Relationship type"
                size="small"
                value={draft.relationshipType}
                onChange={(e) => onRelationshipType(e.target.value)}
              />
            </div>
            <EndpointEditor
              title="Source endpoint"
              endpoint={draft.source}
              headers={draft.headers}
              onChange={(patch) => onEndpoint('source', patch)}
            />
            <EndpointEditor
              title="Target endpoint"
              endpoint={draft.target}
              headers={draft.headers}
              onChange={(patch) => onEndpoint('target', patch)}
            />
            {draft.headers.length > 0 && (
              <div className="flex flex-col gap-2 rounded-md border p-3">
                <p className="text-sm font-semibold">Property columns</p>
                <p className="text-xs text-content2">
                  Optional: map link file columns to relationship properties. Blank = skip.
                </p>
                <div className="flex max-h-40 flex-col gap-1 overflow-y-auto">
                  {draft.headers.map((header) => (
                    <div key={header} className="flex items-center gap-2 text-sm">
                      <span className="w-1/2 truncate font-mono text-xs text-content2">{header}</span>
                      <input
                        className="w-1/2 rounded-sm border bg-secondary px-2 py-1 font-mono text-xs outline-hidden focus-visible:ring"
                        placeholder={header}
                        value={draft.propertyColumns[header] ?? ''}
                        onChange={(e) => onPropertyColumn(header, e.target.value)}
                      />
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        }
      </CardBody>
    </Card>
  )
}

export function NewImport() {
  const projectId = useStore($currentProjectId)
  const [files, setFiles] = useState<FileDraft[]>([])
  const [runName, setRunName] = useState('')
  const [failurePolicy, setFailurePolicy] = useState<'continue' | 'stop_new_files'>('continue')
  const [isDragging, setIsDragging] = useState(false)
  const [fileError, setFileError] = useState<string | undefined>()
  const [submitting, setSubmitting] = useState(false)
  const [submitError, setSubmitError] = useState<string | undefined>()
  const [uploads, setUploads] = useState<Record<string, UploadState>>({})

  const canStart = files.length > 0 && files.every(isFileValid) && !submitting

  const applyFiles = (list: FileList | File[]) => {
    const incoming = Array.from(list)
    const rejected = incoming.filter((f) => !isSupportedFileName(f.name))
    if (rejected.length) {
      setFileError(
        `Unsupported file type: ${rejected.map((f) => f.name).join(', ')}. Use .csv, .jsonl, .ndjson, .json, or .parquet.`
      )
      return
    }
    setFileError(undefined)
    const drafts = incoming.map(makeDraft)
    setFiles((prev) => [...prev, ...drafts])
    drafts.forEach((draft) => {
      readHeaderColumns(draft.file)
        .then((headers) => {
          setFiles((prev) =>
            prev.map((p) =>
              p.clientFileId === draft.clientFileId ?
                {
                  ...p,
                  headers,
                  headersLoaded: true,
                  source: { ...p.source, column: p.source.column || headers[0] || '' },
                  target: { ...p.target, column: p.target.column || headers[1] || '' }
                }
              : p
            )
          )
        })
        .catch(() => {
          setFiles((prev) =>
            prev.map((p) => (p.clientFileId === draft.clientFileId ? { ...p, headersLoaded: true } : p))
          )
        })
    })
  }

  const removeFile = (clientFileId: string) => {
    setFiles((prev) => prev.filter((f) => f.clientFileId !== clientFileId))
    setUploads((prev) => {
      const next = { ...prev }
      delete next[clientFileId]
      return next
    })
  }

  const patchFile = (clientFileId: string, patch: Partial<FileDraft>) => {
    setFiles((prev) => prev.map((f) => (f.clientFileId === clientFileId ? { ...f, ...patch } : f)))
  }

  const buildManifests = (): ImportFileManifest[] =>
    files.map((d) =>
      d.role === 'records' ?
        {
          clientFileId: d.clientFileId,
          fileName: d.fileName,
          size: d.size,
          format: d.format,
          role: 'records' as const,
          rootLabel: d.rootLabel.trim()
        }
      : {
          clientFileId: d.clientFileId,
          fileName: d.fileName,
          size: d.size,
          format: d.format,
          role: 'links' as const,
          linkSpec: {
            version: 1 as const,
            role: 'links' as const,
            endpoints: [
              { ...d.source, direction: 'source' as const },
              { ...d.target, direction: 'target' as const }
            ],
            relationshipType: d.relationshipType.trim(),
            ...(Object.values(d.propertyColumns).some((v) => v.trim()) ?
              {
                propertyColumns: Object.fromEntries(
                  Object.entries(d.propertyColumns)
                    .filter(([, v]) => v.trim())
                    .map(([k, v]) => [k, v.trim()])
                )
              }
            : {})
          }
        }
    )

  const handleStart = async () => {
    if (!canStart || !projectId) return
    setSubmitting(true)
    setSubmitError(undefined)
    setUploads(Object.fromEntries(files.map((f) => [f.clientFileId, { phase: 'idle', progress: 0 }])))
    try {
      const manifests = buildManifests()
      const created = await rushDBInstance.imports.create({
        name: runName.trim() || undefined,
        failurePolicy,
        files: manifests
      })
      const runId = created.runId
      const fileIdByClient = new Map(created.files.map((f) => [f.clientFileId, f.fileId]))

      for (const draft of files) {
        const fileId = fileIdByClient.get(draft.clientFileId)
        if (!fileId) {
          setUploads((prev) => ({ ...prev, [draft.clientFileId]: { phase: 'error', progress: 0 } }))
          continue
        }
        const manifest = manifests.find((m) => m.clientFileId === draft.clientFileId)!
        const manifestWithId = { ...manifest, fileId }
        setUploads((prev) => ({ ...prev, [draft.clientFileId]: { phase: 'uploading', progress: 0 } }))
        try {
          const initiated = await rushDBInstance.imports.uploads.initiate(runId, fileId)
          const direct = Boolean((initiated as { directUpload?: boolean } | undefined)?.directUpload)
          // The probing initiate above created an upload; abandon it and let the chosen
          // upload path manage its own lifecycle to avoid leaving a dangling upload.
          if (initiated) {
            await rushDBInstance.imports.uploads
              .abort(runId, fileId, initiated.uploadId)
              .catch(() => undefined)
          }
          if (direct) {
            await rushDBInstance.imports.uploadFileDirect(runId, manifestWithId, draft.file, {
              onProgress: (p) => {
                const percent = p.totalBytes ? Math.round((p.bytesUploaded / p.totalBytes) * 100) : 0
                setUploads((prev) => ({
                  ...prev,
                  [draft.clientFileId]: { phase: 'uploading', progress: percent }
                }))
              }
            })
          } else {
            await rushDBInstance.imports.uploadContent(runId, manifestWithId, draft.file)
          }
          setUploads((prev) => ({ ...prev, [draft.clientFileId]: { phase: 'complete', progress: 100 } }))
        } catch {
          setUploads((prev) => ({ ...prev, [draft.clientFileId]: { phase: 'error', progress: 0 } }))
          throw new Error(`Upload failed for ${draft.fileName}`)
        }
      }

      await rushDBInstance.imports.start(runId)
      $router.open(getRoutePath('projectImportsRun', { id: projectId, runId }))
    } catch (error) {
      setSubmitError(error instanceof Error ? error.message : 'Import could not be started')
      setSubmitting(false)
    }
  }

  return (
    <div className="mt-4 flex flex-col gap-6">
      <div className="flex flex-col gap-3">
        <h2 className="text-lg font-semibold text-content">New import</h2>
        <p className="text-sm text-content2">
          Add one or more files, choose whether each holds records or relationships, then start the run.
        </p>
      </div>

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <TextField
          label="Run name"
          size="small"
          placeholder="e.g. Q3 product data"
          value={runName}
          onChange={(e) => setRunName(e.target.value)}
        />
        <SelectField
          label="On failure"
          size="small"
          value={failurePolicy}
          onChange={(e) => setFailurePolicy(e.target.value as 'continue' | 'stop_new_files')}
          options={[
            { value: 'continue', label: 'Continue with remaining files' },
            { value: 'stop_new_files', label: 'Stop starting new files' }
          ]}
        />
      </div>

      <div>
        <input
          type="file"
          multiple
          accept={ACCEPT}
          className="hidden"
          id="import-files-input"
          onChange={(e) => {
            if (e.target.files) applyFiles(e.target.files)
            e.target.value = ''
          }}
        />
        <div
          className={cn(
            'border-border flex w-full flex-col items-center justify-center gap-3 rounded-xl border border-dashed bg-transparent px-6 py-10 text-center transition-all',
            isDragging && 'border-accent bg-accent/5 ring-2 ring-accent/30'
          )}
          onDragOver={(e) => {
            e.preventDefault()
            setIsDragging(true)
          }}
          onDragLeave={(e) => {
            e.preventDefault()
            setIsDragging(false)
          }}
          onDrop={(e) => {
            e.preventDefault()
            setIsDragging(false)
            if (e.dataTransfer.files) applyFiles(e.dataTransfer.files)
          }}
        >
          <div className="flex h-14 w-14 items-center justify-center rounded-full bg-secondary text-content">
            <UploadCloud size={24} />
          </div>
          <div>
            <p className="text-lg font-semibold">Drag and drop import files</p>
            <p className="mt-1 text-sm text-content2">
              CSV, JSONL, NDJSON, JSON, or Parquet. Multiple files allowed.
            </p>
          </div>
          <label htmlFor="import-files-input">
            <Button as="span" variant="outline" size="small" className="mt-2 cursor-pointer">
              <Plus size={16} />
              Browse files
            </Button>
          </label>
          {fileError && <p className="text-sm text-danger">{fileError}</p>}
        </div>
      </div>

      {files.length > 0 && (
        <div className="flex flex-col gap-4">
          {files.map((draft) => (
            <FileEditor
              key={draft.clientFileId}
              draft={draft}
              onRemove={() => removeFile(draft.clientFileId)}
              onRoleChange={(role) => patchFile(draft.clientFileId, { role })}
              onRootLabel={(value) => patchFile(draft.clientFileId, { rootLabel: value })}
              onRelationshipType={(value) => patchFile(draft.clientFileId, { relationshipType: value })}
              onEndpoint={(endpoint, patch) =>
                patchFile(draft.clientFileId, {
                  [endpoint]: { ...draft[endpoint], ...patch }
                } as Partial<FileDraft>)
              }
              onPropertyColumn={(column, property) =>
                patchFile(draft.clientFileId, {
                  propertyColumns: { ...draft.propertyColumns, [column]: property }
                })
              }
            />
          ))}
        </div>
      )}

      {submitError && <p className="text-sm text-danger">{submitError}</p>}

      {submitting && (
        <div className="flex flex-col gap-2">
          <h3 className="text-sm font-semibold text-content">Uploading files…</h3>
          {files.map((draft) => {
            const state = uploads[draft.clientFileId] ?? { phase: 'idle', progress: 0 }
            return (
              <div key={draft.clientFileId} className="flex items-center gap-3 text-sm">
                <span className="min-w-0 flex-1 truncate">{draft.fileName}</span>
                <div className="h-1.5 w-32 overflow-hidden rounded-full bg-secondary">
                  <div
                    className={cn(
                      'h-full rounded-full transition-all',
                      state.phase === 'error' ? 'bg-danger'
                      : state.phase === 'complete' ? 'bg-success'
                      : 'bg-accent'
                    )}
                    style={{ width: `${state.progress}%` }}
                  />
                </div>
                <span className="w-24 text-right font-mono text-xs text-content2">
                  {state.phase === 'error' ?
                    'failed'
                  : state.phase === 'complete' ?
                    'done'
                  : `${state.progress}%`}
                </span>
              </div>
            )
          })}
        </div>
      )}

      <div className="flex items-center justify-end gap-2">
        <Button as="a" href={getRoutePath('projectImports', { id: projectId! })} variant="secondary">
          Cancel
        </Button>
        <Button onClick={handleStart} loading={submitting} disabled={!canStart} variant="primary">
          {submitting ? 'Starting…' : 'Start import'}
        </Button>
      </div>
    </div>
  )
}
