import { useMemo, useRef } from 'react'
import { useQuery } from '@tanstack/react-query'

import { ExternalLink, RefreshCw } from 'lucide-react'

import type { Project } from '~/features/projects/types'

import { Button } from '~/elements/Button'
import { Editor } from '~/elements/Editor'
import { NothingFound } from '~/elements/NothingFound'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { Tab, Tabs, TabsContent, TabsList } from '~/elements/Tabs'
import { api } from '~/lib/api'
import { queryKeys } from '~/lib/queryKeys'
import { cn } from '~/lib/utils'

const LIVE_SCHEMA_DOCS_URL = 'https://docs.rushdb.com/learn/records-and-queries/relationships'

export function ProjectLiveSchema({ projectId }: { projectId: Project['id'] }) {
  // Set just before a manual refresh so the next fetch bypasses the 1-hour server cache.
  const forceRef = useRef(false)

  const schemaQuery = useQuery({
    queryKey: queryKeys.projects.schema(projectId),
    queryFn: () => api.ai.getSchema({ projectId, force: forceRef.current }),
    enabled: !!projectId
  })

  const markdownQuery = useQuery({
    queryKey: queryKeys.projects.schemaMarkdown(projectId),
    queryFn: () => api.ai.getSchemaMarkdown({ projectId, force: forceRef.current }),
    enabled: !!projectId
  })

  const jsonValue = useMemo(
    () => (schemaQuery.data ? JSON.stringify(schemaQuery.data, null, 2) : ''),
    [schemaQuery.data]
  )

  const refreshing = schemaQuery.isFetching || markdownQuery.isFetching

  const refresh = async () => {
    forceRef.current = true
    try {
      await Promise.all([schemaQuery.refetch(), markdownQuery.refetch()])
    } finally {
      forceRef.current = false
    }
  }

  const loading = schemaQuery.isPending || markdownQuery.isPending
  const failed = schemaQuery.isError || markdownQuery.isError

  return (
    <>
      <PageHeader className="items-start" contained>
        <div className="flex max-w-3xl flex-col gap-2">
          <PageTitle>Live Schema</PageTitle>
          <p className="text-content2 text-sm leading-6">
            The current graph schema RushDB infers for this project — labels, properties with value ranges,
            and relationships. This is the same schema served to AI agents and the API.
          </p>
          <a
            className="text-content2 hover:text-content inline-flex w-fit items-center gap-2 text-sm transition"
            href={LIVE_SCHEMA_DOCS_URL}
            rel="noreferrer"
            target="_blank"
          >
            Read the docs <ExternalLink className="h-3.5 w-3.5" />
          </a>
        </div>
      </PageHeader>
      <PageContent contained>
        <Tabs className="flex flex-col gap-3" defaultValue="json">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <TabsList>
              <Tab value="json">JSON</Tab>
              <Tab value="md">Markdown</Tab>
            </TabsList>
            <Button disabled={refreshing} onClick={refresh} size="small" variant="secondary">
              <RefreshCw className={cn(refreshing && 'animate-spin')} />
              Refresh
            </Button>
          </div>

          {failed ?
            <NothingFound title="Failed to load schema" />
          : <>
              <TabsContent value="json">
                <div className="bg-fill overflow-hidden rounded-md border">
                  <Editor
                    defaultLanguage="json"
                    height="70vh"
                    lineNumbers="off"
                    readOnly
                    theme="vs-dark"
                    value={loading ? '// Loading schema…' : jsonValue}
                  />
                </div>
              </TabsContent>
              <TabsContent value="md">
                <div className="bg-fill overflow-hidden rounded-md border">
                  <Editor
                    defaultLanguage="markdown"
                    height="70vh"
                    lineNumbers="off"
                    readOnly
                    theme="vs-dark"
                    value={loading ? 'Loading schema…' : (markdownQuery.data ?? '')}
                  />
                </div>
              </TabsContent>
            </>
          }
        </Tabs>
      </PageContent>
    </>
  )
}
