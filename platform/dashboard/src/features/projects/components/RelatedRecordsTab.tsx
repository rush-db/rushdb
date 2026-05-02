import { ArrowLeft, ArrowRight, ArrowUpRight } from 'lucide-react'
import { useMemo } from 'react'
import { Relation } from '@rushdb/javascript-sdk'

import { Banner } from '~/elements/Banner'
import { NothingFound } from '~/elements/NothingFound'
import { Spinner } from '~/elements/Spinner'
import { IconButton } from '~/elements/IconButton'
import { Label } from '~/elements/Label'

import { useCurrentRecordQuery, useCurrentRecordRelatedQuery } from '../hooks/useProjectQueries'
import { $sheetRecordId } from '../stores/id'

interface RelationGroup {
  type: string
  incoming: Relation[]
  outgoing: Relation[]
}

export function RelatedRecordsTab() {
  const { data: relationsResult, isPending: loading } = useCurrentRecordRelatedQuery()
  const relations = relationsResult?.data
  const { data: record } = useCurrentRecordQuery()

  const groupedRelations = useMemo(() => {
    if (!relations || !record) return []

    const grouped = new Map<string, RelationGroup>()

    relations.forEach((relation) => {
      if (!grouped.has(relation.type)) {
        grouped.set(relation.type, { type: relation.type, incoming: [], outgoing: [] })
      }

      const group = grouped.get(relation.type)!
      if (relation.targetId === record.__id) {
        group.incoming.push(relation)
      } else {
        group.outgoing.push(relation)
      }
    })

    return Array.from(grouped.values())
  }, [relations, record])

  if (!record || loading) {
    return <Banner image={<Spinner />} />
  }

  if (!relations || relations.length === 0) {
    return <NothingFound title="This Record doesn't have any related Records" />
  }

  return (
    <div className="divide-stroke-tertiary flex flex-col divide-y">
      {groupedRelations.map(({ type, incoming, outgoing }) => (
        <div key={type}>
          <div className="bg-fill2 px-5 py-2 text-xs text-content-secondary">
            {type}
          </div>

          <div className="divide-stroke-tertiary divide-y">
            {incoming.map(({ sourceLabel, sourceId }) => (
              <div key={sourceId} className="hover:bg-secondary flex items-center gap-3 px-5 py-3">
                <ArrowLeft className="h-4 w-4 shrink-0 text-green-400" />
                <div className="flex flex-1 min-w-0 flex-col gap-0.5">
                  <Label>{sourceLabel}</Label>
                  <span className="text-content-secondary font-mono text-xs truncate">{sourceId}</span>
                </div>
                <IconButton
                  aria-label="open record"
                  title="Open record"
                  variant="ghost"
                  size="small"
                  onClick={() => $sheetRecordId.set(sourceId)}
                >
                  <ArrowUpRight />
                </IconButton>
              </div>
            ))}

            {outgoing.map(({ targetLabel, targetId }) => (
              <div key={targetId} className="hover:bg-secondary flex items-center gap-3 px-5 py-3">
                <ArrowRight className="h-4 w-4 shrink-0 text-blue-400" />
                <div className="flex flex-1 min-w-0 flex-col gap-0.5">
                  <Label>{targetLabel}</Label>
                  <span className="text-content-secondary font-mono text-xs truncate">{targetId}</span>
                </div>
                <IconButton
                  aria-label="open record"
                  title="Open record"
                  variant="ghost"
                  size="small"
                  onClick={() => $sheetRecordId.set(targetId)}
                >
                  <ArrowUpRight />
                </IconButton>
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  )
}
