import type { SavedQuery } from '~/features/saved-queries/types'

import { ExternalLink, MoreVertical, PlayIcon, Trash2 } from 'lucide-react'

import { Button } from '~/elements/Button'
import { Card } from '~/elements/Card'
import { ConfirmDialog } from '~/elements/ConfirmDialog'
import { IconButton } from '~/elements/IconButton'
import { Menu, MenuItem } from '~/elements/Menu'
import { NothingFound } from '~/elements/NothingFound'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { Skeleton } from '~/elements/Skeleton'
import { applySavedQuery } from '~/features/projects/stores/records-search'
import { useDeleteSavedQueryMutation, useSavedQueriesQuery } from '~/features/saved-queries/hooks'
import { formatIsoToLocalDateTime } from '~/lib/formatters'
import { cn } from '~/lib/utils'

const SAVED_QUERIES_DOCS_URL = 'https://docs.rushdb.com/concepts/search/introduction'

const MODE_LABELS: Record<SavedQuery['searchMode'], string> = {
  manual: 'Builder',
  ai: 'Smart',
  semantic: 'Semantic'
}

const MODE_COLORS: Record<SavedQuery['searchMode'], string> = {
  manual: 'text-badge-blue border-badge-blue/30 bg-badge-blue/10',
  ai: 'text-badge-green border-badge-green/30 bg-badge-green/10',
  semantic: 'text-badge-orange border-badge-orange/30 bg-badge-orange/10'
}

function SavedQueryRow({ savedQuery }: { savedQuery: SavedQuery }) {
  const { mutate: deleteSavedQuery } = useDeleteSavedQueryMutation()

  return (
    <li className="flex items-center gap-3 px-3 py-3 sm:gap-4 sm:px-4">
      <div className="flex min-w-0 flex-1 flex-col">
        <span className="mb-1 flex items-center gap-2">
          <span className="text-content truncate font-medium">{savedQuery.name}</span>
          <span
            className={cn(
              'inline-flex shrink-0 items-center rounded-md border px-1.5 py-0.5 text-xs font-medium',
              MODE_COLORS[savedQuery.searchMode]
            )}
          >
            {MODE_LABELS[savedQuery.searchMode]}
          </span>
        </span>
        {savedQuery.prompt ?
          <span className="text-content2 truncate text-sm">{savedQuery.prompt}</span>
        : null}
        <span className="text-content3 text-xs">
          {formatIsoToLocalDateTime(savedQuery.createdAt)}
          {savedQuery.createdBy ? ` · ${savedQuery.createdBy}` : ''}
        </span>
      </div>

      <Button onClick={() => applySavedQuery(savedQuery)} size="small" variant="outline">
        <PlayIcon />
        Open
      </Button>

      <Menu
        trigger={
          <IconButton aria-label="more" title="More" variant="ghost">
            <MoreVertical />
          </IconButton>
        }
        align="end"
      >
        <ConfirmDialog
          handler={() => {
            deleteSavedQuery(savedQuery.id)
            return Promise.resolve()
          }}
          trigger={
            <MenuItem dropdown icon={<Trash2 />} variant="danger">
              Delete
            </MenuItem>
          }
          description={`The saved query "${savedQuery.name}" will be permanently deleted. Are you sure?`}
          title="Delete saved query"
        />
      </Menu>
    </li>
  )
}

export function ProjectSavedQueries() {
  const { data: savedQueries, isPending: loading } = useSavedQueriesQuery()

  return (
    <>
      <PageHeader className="items-start" contained>
        <div className="flex max-w-3xl flex-col gap-2">
          <PageTitle>Saved Queries</PageTitle>
          <p className="text-content2 text-sm leading-6">
            Reusable searches saved from the Records page. Open one to re-run it with its original search
            mode, prompt, and filters.
          </p>
          <a
            className="text-content2 hover:text-content inline-flex w-fit items-center gap-2 text-sm transition"
            href={SAVED_QUERIES_DOCS_URL}
            rel="noreferrer"
            target="_blank"
          >
            Read the docs <ExternalLink className="h-3.5 w-3.5" />
          </a>
        </div>
      </PageHeader>
      <PageContent contained>
        {!loading && savedQueries && savedQueries.length < 1 ?
          <NothingFound title="No saved queries yet. Save one from the Records page using View Query → Save query." />
        : <Card>
            <ul className="flex flex-col divide-y">
              {savedQueries?.map((savedQuery) => (
                <SavedQueryRow key={savedQuery.id} savedQuery={savedQuery} />
              ))}
              {loading ?
                <li className="px-4 py-3">
                  <Skeleton enabled>Loading saved queries…</Skeleton>
                </li>
              : null}
            </ul>
          </Card>
        }
      </PageContent>
    </>
  )
}
