import type { SearchQuery } from '@rushdb/javascript-sdk'
import type { SdkLanguage } from '~/features/onboarding/types'

import { useStore } from '@nanostores/react'
import { useEffect, useMemo, useState } from 'react'

import { Button, CopyButton } from '~/elements/Button'
import { CodeEditorSnippet } from '~/elements/CodeEditorSnippet'
import { Dialog, DialogFooter, DialogTitle } from '~/elements/Dialog'
import { Editor } from '~/elements/Editor'
import { TextField } from '~/elements/Input'
import { toast } from '~/elements/Toast'
import { $currentProjectRecordsSkip } from '~/features/projects/stores/current-project'
import { $currentProjectId } from '~/features/projects/stores/id'
import {
  $aiSearchPrompt,
  $aiSearchQuery,
  $draftSearchQuery,
  $recordsSearchMode,
  $semanticSearchIndexId,
  $semanticSearchPrompt,
  $searchQueryModalOpen,
  setAiSearchQuery
} from '~/features/projects/stores/records-search'
import {
  useCurrentManualRecordsSearchQuery,
  useProjectIndexesQuery
} from '~/features/projects/hooks/useProjectQueries'
import { useSaveQueryMutation } from '~/features/saved-queries/hooks'
import { Tab, Tabs, TabsContent, TabsList } from '~/elements/Tabs'
import { SDK_LANGUAGE_CONFIG } from '~/features/onboarding/components/SelectSdkLanguage'

function formatQuery(query: unknown) {
  return JSON.stringify(query ?? {}, null, 2)
}

function parseJson(value: string, fallback: unknown) {
  try {
    return JSON.parse(value)
  } catch {
    return fallback
  }
}

function buildCurlSnippet(endpoint: string, payload: unknown) {
  return `curl -X POST "$RUSHDB_API_URL${endpoint}" \\
  -H "Authorization: Bearer $RUSHDB_API_KEY" \\
  -H "Content-Type: application/json" \\
  --data @- <<'JSON'
${formatQuery(payload)}
JSON`
}

function buildTypeScriptSnippet(mode: 'records' | 'semantic', payload: unknown) {
  const method = mode === 'semantic' ? 'ai.search' : 'records.find'
  return `import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB(process.env.RUSHDB_API_KEY!)

const result = await db.${method}(${formatQuery(payload)})

console.log(result.data)`
}

function buildPythonSnippet(mode: 'records' | 'semantic', payload: unknown) {
  const method = mode === 'semantic' ? 'ai.search' : 'records.find'
  return `import os
import json
from rushdb import RushDB

db = RushDB(os.environ["RUSHDB_API_KEY"])
payload = json.loads(r'''${formatQuery(payload)}''')

response = db.${method}(payload)

print(response.data)`
}

export function SearchQueryModal() {
  const open = useStore($searchQueryModalOpen)
  const mode = useStore($recordsSearchMode)
  const projectId = useStore($currentProjectId)
  const aiPrompt = useStore($aiSearchPrompt)
  const aiQuery = useStore($aiSearchQuery)
  const draftQuery = useStore($draftSearchQuery)
  const semanticIndexId = useStore($semanticSearchIndexId)
  const semanticPrompt = useStore($semanticSearchPrompt)
  const { data: indexes } = useProjectIndexesQuery({ enabled: open })
  const semanticIndex = indexes?.find((index) => index.id === semanticIndexId)
  const manualQuery = useCurrentManualRecordsSearchQuery()
  const semanticPayload =
    semanticIndex ?
      {
        labels: [semanticIndex.label],
        propertyName: semanticIndex.propertyName,
        query: semanticPrompt,
        sourceType: semanticIndex.sourceType,
        skip: manualQuery.skip,
        limit: manualQuery.limit
      }
    : undefined
  const activeQuery =
    mode === 'semantic' ? semanticPayload
    : draftQuery ? draftQuery
    : mode === 'ai' && aiQuery ? aiQuery
    : manualQuery
  const formattedQuery = useMemo(() => formatQuery(activeQuery), [activeQuery])

  const [value, setValue] = useState(formattedQuery)
  const [error, setError] = useState<string | undefined>()
  const [activeTab, setActiveTab] = useState<'json' | SdkLanguage>('json')
  const [saveOpen, setSaveOpen] = useState(false)
  const [saveName, setSaveName] = useState('')
  const { mutateAsync: saveQuery, isPending: saving } = useSaveQueryMutation()

  useEffect(() => {
    if (open) {
      setValue(formattedQuery)
      setError(undefined)
      setActiveTab('json')
    }
  }, [formattedQuery, open])

  const parsedValue = useMemo(() => parseJson(value, activeQuery), [activeQuery, value])
  const snippetMode = mode === 'semantic' ? 'semantic' : 'records'
  const endpoint = mode === 'semantic' ? '/api/v1/ai/search' : '/api/v1/records/search'
  const snippets = useMemo(
    () => ({
      typescript: {
        code: buildTypeScriptSnippet(snippetMode, parsedValue),
        language: 'typescript',
        title: 'TypeScript SDK'
      },
      python: {
        code: buildPythonSnippet(snippetMode, parsedValue),
        language: 'python',
        title: 'Python SDK'
      },
      shell: {
        code: buildCurlSnippet(endpoint, parsedValue),
        language: 'shell',
        title: 'REST API'
      }
    }),
    [endpoint, parsedValue, snippetMode]
  )

  const apply = () => {
    try {
      const parsed = JSON.parse(value)
      $currentProjectRecordsSkip.set(0)
      if (mode === 'semantic') {
        const matchingIndex = indexes?.find(
          (index) =>
            Array.isArray(parsed.labels) &&
            parsed.labels[0] === index.label &&
            parsed.propertyName === index.propertyName &&
            (!parsed.sourceType || parsed.sourceType === index.sourceType)
        )

        if (!matchingIndex) {
          throw new Error('Semantic payload must reference an existing semantic index.')
        }

        $semanticSearchIndexId.set(matchingIndex.id)
        $semanticSearchPrompt.set(typeof parsed.query === 'string' ? parsed.query : '')
      } else {
        setAiSearchQuery(parsed as SearchQuery)
        $draftSearchQuery.set(undefined)
      }
      $searchQueryModalOpen.set(false)
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Invalid JSON')
    }
  }

  const save = async () => {
    let searchQuery: SearchQuery
    try {
      searchQuery = JSON.parse(value) as SearchQuery
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Invalid JSON')
      return
    }

    try {
      await saveQuery({
        name: saveName.trim(),
        searchMode: mode,
        searchQuery,
        prompt:
          mode === 'ai' ? aiPrompt
          : mode === 'semantic' ? semanticPrompt
          : undefined,
        semanticIndexId: mode === 'semantic' ? semanticIndexId : undefined
      })
      toast({ title: 'Query saved', description: `"${saveName.trim()}" is now in Saved Queries.` })
      setSaveOpen(false)
      setSaveName('')
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to save query')
    }
  }

  return (
    <Dialog className="sm:max-w-4xl" onOpenChange={$searchQueryModalOpen.set} open={open}>
      <DialogTitle className="mb-4">Search Query</DialogTitle>
      <Tabs value={activeTab} onValueChange={(nextValue) => setActiveTab(nextValue as typeof activeTab)}>
        <div className="mb-3">
          <TabsList>
            <Tab value="json">JSON</Tab>
            {(['typescript', 'python', 'shell'] as const).map((language) => {
              const { label, Icon } = SDK_LANGUAGE_CONFIG[language]

              return (
                <Tab key={language} value={language}>
                  <span className="flex items-center gap-1.5">
                    <Icon />
                    {label}
                  </span>
                </Tab>
              )
            })}
          </TabsList>
        </div>
        <TabsContent value="json">
          <div className="overflow-hidden rounded-md border bg-fill">
            <div className="flex items-center justify-between gap-3 border-b px-3 py-2">
              <span className="text-sm font-medium text-content2">Editable SearchQuery payload</span>
              <CopyButton size="xsmall" text={value} variant="outline">
                Copy
              </CopyButton>
            </div>
            <Editor
              defaultLanguage="json"
              height="420px"
              onChange={(nextValue) => {
                setValue(nextValue ?? '')
                setError(undefined)
              }}
              value={value}
            />
          </div>
        </TabsContent>
        <TabsContent value="typescript">
          <CodeEditorSnippet {...snippets.typescript} height="420px" />
        </TabsContent>
        <TabsContent value="python">
          <CodeEditorSnippet {...snippets.python} height="420px" />
        </TabsContent>
        <TabsContent value="shell">
          <CodeEditorSnippet {...snippets.shell} height="420px" />
        </TabsContent>
      </Tabs>
      {error ?
        <p className="text-sm text-danger">{error}</p>
      : <p className="text-sm text-content2">
          Apply executes the JSON payload. Code snippets use `RUSHDB_API_KEY`
          {projectId ? ` for project ${projectId}.` : '.'}
        </p>
      }
      <DialogFooter className="mt-4">
        <Button onClick={() => $searchQueryModalOpen.set(false)} variant="outline">
          Cancel
        </Button>
        <Button onClick={() => setSaveOpen(true)} variant="outline">
          Save query
        </Button>
        <Button onClick={apply} variant="primary">
          Apply query
        </Button>
      </DialogFooter>

      <Dialog
        className="sm:max-w-md"
        onOpenChange={(next) => {
          setSaveOpen(next)
          if (!next) setSaveName('')
        }}
        open={saveOpen}
      >
        <DialogTitle>Save query</DialogTitle>
        <p className="mb-3 text-sm text-content2">
          Save the current{' '}
          {mode === 'ai' ?
            'Smart'
          : mode === 'semantic' ?
            'Semantic'
          : 'Builder'}{' '}
          query so you can re-run it later from Saved Queries.
        </p>
        <TextField
          autoFocus
          label="Name"
          onChange={(event) => setSaveName(event.target.value)}
          placeholder="e.g. Top planets by battles"
          value={saveName}
        />
        <DialogFooter className="mt-4">
          <Button onClick={() => setSaveOpen(false)} variant="outline">
            Cancel
          </Button>
          <Button disabled={!saveName.trim()} loading={saving} onClick={save} variant="primary">
            Save
          </Button>
        </DialogFooter>
      </Dialog>
    </Dialog>
  )
}
