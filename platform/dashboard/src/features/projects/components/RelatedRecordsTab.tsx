import { useStore } from '@nanostores/react'
import { Banner } from '~/elements/Banner'
import { NothingFound } from '~/elements/NothingFound'
import { Spinner } from '~/elements/Spinner'
import { $currentRecord, $currentRelatedRecords } from '../stores/current-record'
import { useMemo } from 'react'
import { ArrowRight, ArrowLeft } from 'lucide-react'
import { Relation } from '@rushdb/javascript-sdk'

interface RelationGroup {
  type: string
  incoming: Relation[]
  outgoing: Relation[]
}

export function RelatedRecordsTab() {
  const { data: relations, loading } = useStore($currentRelatedRecords)
  const { data: record } = useStore($currentRecord)

  const groupedRelations = useMemo(() => {
    if (!relations || !record) return []

    const grouped = new Map<string, RelationGroup>()

    relations.forEach((relation) => {
      if (!grouped.has(relation.type)) {
        grouped.set(relation.type, { type: relation.type, incoming: [], outgoing: [] })
      }

      const group = grouped.get(relation.type)!
      if (relation.targetId === record!.__id) {
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
    <div className="mt-3 p-6">
      <div className="space-y-4">
        {groupedRelations.map(({ type, incoming, outgoing }) => (
          <div key={type} className="border-b pb-6">
            <h3 className="text-md mb-2 font-semibold text-gray-300">{type}</h3>

            {incoming.length > 0 && (
              <div>
                <h4 className="mb-1 text-sm text-gray-400">Incoming</h4>
                <ul className="space-y-1">
                  {incoming.map(({ sourceLabel, sourceId }) => (
                    <li key={sourceId} className="flex items-center text-gray-300">
                      <ArrowLeft className="mr-2 h-4 w-4 text-green-400" />
                      <span className="text-sm">
                        {sourceLabel} ({sourceId})
                      </span>
                    </li>
                  ))}
                </ul>
              </div>
            )}

            {outgoing.length > 0 && (
              <div className="mt-2">
                <h4 className="mb-1 text-sm text-gray-400">Outgoing</h4>
                <ul className="space-y-1">
                  {outgoing.map(({ targetLabel, targetId }) => (
                    <li key={targetId} className="flex items-center text-gray-300">
                      <ArrowRight className="mr-2 h-4 w-4 text-blue-400" />
                      <span className="text-sm">
                        {targetLabel} ({targetId})
                      </span>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  )
}
