import type { FC } from 'react'
import React, { useCallback, useMemo } from 'react'

import { useStore } from '@nanostores/react'

import {
  useFilteredRecordsQuery,
  useGraphRecordRelationsQuery,
  useProjectFieldsQuery,
  useProjectIndexesQuery,
  useProjectLabelsQuery
} from '~/features/projects/hooks/useProjectQueries'
import {
  $sheetProperty,
  openPropertySheet,
  openRecordSheet,
  type PropertySheetData
} from '~/features/projects/stores/id.ts'
import { $tourAllowed, $tourStep } from '~/features/tour/stores/tour'
import { getLabelColor } from '~/features/labels'
import type { DBRecord, DBRecordInstance } from '@rushdb/javascript-sdk'
import { type Relation } from '@rushdb/javascript-sdk'

import { GraphCanvas, type GraphNode, type GraphOutput } from './GraphCanvas'

export type { GraphMode, GraphNode, GraphLink, GraphOutput } from './GraphCanvas'

function inferPropertyType(value: unknown): string {
  if (value === null) return 'null'
  if (Array.isArray(value)) {
    if (value.length === 0) return 'string'
    return inferPropertyType(value[0])
  }
  if (typeof value === 'number') return 'number'
  if (typeof value === 'boolean') return 'boolean'
  if (typeof value === 'string') {
    return /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/.test(value) ? 'datetime' : 'string'
  }
  return 'string'
}

function propertyNodeKey(label: string, name: string, type: string) {
  return `property:${label}:${name}:${type}`
}

function recordNodeKey(recordId: string) {
  return `record:${recordId}`
}

function createGraphData({
  relations,
  records,
  labels,
  indexedByLabelAndProperty
}: {
  relations: Relation[]
  records?: DBRecordInstance[]
  labels?: Record<string, unknown>
  indexedByLabelAndProperty: Set<string>
}): GraphOutput {
  const nodeMap = new Map<string, GraphNode>()
  const linkMap = new Map<string, GraphOutput['links'][number]>()

  records?.forEach((recordInstance) => {
    const record = recordInstance.data as DBRecord
    const recordId = record.__id
    const label = record.__label
    const nodeId = recordNodeKey(recordId)

    nodeMap.set(nodeId, {
      id: nodeId,
      kind: 'record',
      label,
      color: getLabelColor(label, Object.keys(labels ?? {}).indexOf(label)),
      recordId
    })

    const rawTypes = record.__proptypes ?? {}
    const typeEntries =
      Object.entries(rawTypes).length ?
        Object.entries(rawTypes)
      : Object.entries(record)
          .filter(([key]) => !key.startsWith('__'))
          .map(([name, value]) => [name, inferPropertyType(value)])

    typeEntries.forEach(([propertyName, type]) => {
      const propertyType = String(type)
      const propertyId = propertyNodeKey(label, propertyName, propertyType)
      const indexKey = `${label}:${propertyName}`

      const existing = nodeMap.get(propertyId)
      if (!existing) {
        nodeMap.set(propertyId, {
          id: propertyId,
          kind: 'property',
          label: propertyName,
          // Neutral slate — property nodes carry no brand color.
          color: '#94a3b8',
          propertyName,
          propertyType,
          propertyLabels: [label],
          vectorIndexed: indexedByLabelAndProperty.has(indexKey),
          connectedRecordIds: [recordId]
        })
      } else {
        const nextLabels = new Set([...(existing.propertyLabels ?? []), label])
        const nextRecords = new Set([...(existing.connectedRecordIds ?? []), recordId])
        existing.propertyLabels = Array.from(nextLabels)
        existing.connectedRecordIds = Array.from(nextRecords)
        existing.vectorIndexed = existing.vectorIndexed || indexedByLabelAndProperty.has(indexKey)
      }

      const propertyLinkId = `property-link:${propertyId}->${nodeId}`
      linkMap.set(propertyLinkId, {
        id: propertyLinkId,
        kind: 'property-value',
        source: propertyId,
        target: nodeId,
        relationType: '__RUSHDB__RELATION__VALUE__'
      })
    })
  })

  relations.forEach((relation) => {
    const source = recordNodeKey(relation.sourceId)
    const target = recordNodeKey(relation.targetId)

    if (!nodeMap.has(source) || !nodeMap.has(target)) {
      return
    }

    const relationId = `relation:${relation.sourceId}:${relation.targetId}:${relation.type}`
    linkMap.set(relationId, {
      id: relationId,
      kind: 'record-relation',
      source,
      target,
      relationType: relation.type
    })
  })

  return {
    nodes: Array.from(nodeMap.values()),
    links: Array.from(linkMap.values())
  }
}

export const GraphView: FC = () => {
  const { data: recordsResult } = useFilteredRecordsQuery()
  const { data: labels } = useProjectLabelsQuery()
  const { data: indexes } = useProjectIndexesQuery()
  const { data: fields } = useProjectFieldsQuery()
  const records = recordsResult?.data
  const recordIds = useMemo(
    () =>
      (records ?? [])
        .map((recordInstance) => (recordInstance.data as DBRecord).__id)
        .filter((id): id is string => typeof id === 'string' && id.length > 0),
    [records]
  )
  const { data: relationsResult } = useGraphRecordRelationsQuery(recordIds)
  const relations = relationsResult?.data

  const selectedProperty = useStore($sheetProperty)
  const tourAllowed = useStore($tourAllowed)
  const tourStep = useStore($tourStep)

  const indexedByLabelAndProperty = useMemo(() => {
    const values = new Set<string>()
    indexes?.forEach((index) => {
      values.add(`${index.label}:${index.propertyName}`)
    })
    return values
  }, [indexes])

  const rawGraphData = useMemo(
    () =>
      createGraphData({
        relations: relations ?? [],
        records,
        labels,
        indexedByLabelAndProperty
      }),
    [relations, records, labels, indexedByLabelAndProperty]
  )

  const handleNodeClick = useCallback(
    (node: GraphNode) => {
      if (node.kind === 'record' && node.recordId) {
        openRecordSheet(node.recordId)
        return
      }

      if (node.kind === 'property') {
        const name = node.propertyName ?? node.label
        const type = node.propertyType ?? 'string'
        const field = fields?.find((f) => f.name === name && f.type === type)
        openPropertySheet({
          id: field?.id ?? '',
          name,
          type: type as PropertySheetData['type'],
          recordsCount: field?.recordsCount,
          metadata: field?.metadata,
          vectorIndexed: !!node.vectorIndexed,
          connectedRecordIds: node.connectedRecordIds ?? []
        })
      }
    },
    [fields]
  )

  return (
    <GraphCanvas
      dataTour="records-graph-view"
      graphData={rawGraphData}
      onNodeClick={handleNodeClick}
      tourFitActive={tourAllowed && tourStep === 'recordGraphView'}
    >
      {selectedProperty && (
        <div className="bg-fill/90 border-content/30 absolute right-4 top-20 z-20 max-w-xs rounded-xl border p-3 shadow-xl backdrop-blur-sm">
          <div className="text-xs font-semibold uppercase tracking-wide">Selected Property</div>
          <div className="mt-1 text-sm font-medium">{selectedProperty.name}</div>
          <div className="text-content-secondary text-xs">Alt + click node to hide it</div>
        </div>
      )}

      {relationsResult?.edgeLimitReached && (
        <div className="bg-fill/90 border-content/30 text-content-secondary absolute bottom-4 left-4 z-20 max-w-sm rounded-lg border px-3 py-2 text-xs shadow-xl backdrop-blur-sm">
          Showing first {relationsResult.edgeLimit.toLocaleString()} relationships between loaded records.
        </div>
      )}
    </GraphCanvas>
  )
}
