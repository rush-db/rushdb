'use client'
import { useCallback, useEffect, useRef, useState } from 'react'
import { atom, map, onMount, onNotify } from 'nanostores'

import { DBRecordInstance, Property, Relation } from '@rushdb/javascript-sdk'
import {
  $filteredRecords,
  $filteredRecordsRelations,
  $currentProjectFields,
  $currentProjectFilters,
  $recordsOrderBy,
  $activeLabels,
  $combineFilters
} from './stores/current-project'
import { $currentProjectId } from './stores/id'
import { useStore } from '@nanostores/react'
import { api } from '~/lib/api'
import { queryClient } from '~/lib/queryClient'
import { convertToSearchQuery, filterToSearchOperation } from './utils'
import { Sort, SortDirection } from '~/types'

type GraphNode = {
  id: string
  type: 'record' | 'property'
  name?: string
  label?: string
  propertyType?: string
  val?: number
  proptypes?: Record<string, any>
  neighbors?: GraphNode[]
  links?: GraphLink[]
  // force-graph runtime fields
  x?: number
  y?: number
  z?: number
  [k: string]: any
}
type GraphLink = { source: string | GraphNode; target: string | GraphNode; type: string }

// Constants for graph data fetching
const GRAPH_BATCH_SIZE = 1000
const GRAPH_MAX_RECORDS = 10000
const GRAPH_MAX_BATCHES = GRAPH_MAX_RECORDS / GRAPH_BATCH_SIZE

// Graph-specific stores for fetching large datasets
type GraphRecordsQueryData = { data: unknown[]; total?: number; hasMore: boolean }

function buildGraphRecordsQueryArgs(skip = 0, limit = GRAPH_BATCH_SIZE) {
  const filters = $currentProjectFilters.get()
  const orderBy = $recordsOrderBy.get()
  const labels = $activeLabels.get()
  const combineMode = $combineFilters.get()
  const properties = filters.map(filterToSearchOperation)

  const order = Object.entries(orderBy ?? {}).reduce<Sort>((acc, [key, direction]) => {
    if (key === '__id') {
      return direction as SortDirection
    }
    if (key && direction) {
      // @ts-ignore
      acc[key] = direction as SortDirection
    }
    return acc
  }, {})

  const where =
    combineMode === 'or' ? { $or: convertToSearchQuery(properties) } : convertToSearchQuery(properties)

  return { where, orderBy: order, skip, limit, labels }
}

async function fetchGraphRecords(
  skip = 0,
  limit = GRAPH_BATCH_SIZE
): Promise<GraphRecordsQueryData | undefined> {
  const projectId = $currentProjectId.get()
  if (!projectId) return
  const args = buildGraphRecordsQueryArgs(skip, limit)
  const { data, total } = await api.records.find(args as any, { signal: undefined as any })
  return { data, total, hasMore: (total || 0) > skip + limit }
}

async function fetchGraphRelations(
  skip = 0,
  limit = GRAPH_BATCH_SIZE
): Promise<GraphRecordsQueryData | undefined> {
  const projectId = $currentProjectId.get()
  if (!projectId) return
  const args = buildGraphRecordsQueryArgs(skip, limit)
  const { data, total } = await api.relationships.find({ searchQuery: args } as any)
  return { data, total, hasMore: (total || 0) > skip + limit }
}

// Graph records store that accumulates up to 10k records
export const $graphRecords = map<{
  data: any[]
  loading: boolean
  error?: string
  total?: number
  loadedBatches: number
  hasMore: boolean
}>({
  data: [],
  loading: false,
  error: undefined,
  total: undefined,
  loadedBatches: 0,
  hasMore: true
})

// Graph relations store that accumulates relations
export const $graphRelations = map<{
  data: any[]
  loading: boolean
  error?: string
  total?: number
  loadedBatches: number
  hasMore: boolean
}>({
  data: [],
  loading: false,
  error: undefined,
  total: undefined,
  loadedBatches: 0,
  hasMore: true
})

// Sequential batch loading for records
// @ts-ignore
$graphRecords.loadNextBatch = async () => {
  const currentState = $graphRecords.get()
  if (currentState.loading || !currentState.hasMore || currentState.loadedBatches >= GRAPH_MAX_BATCHES) {
    return
  }

  const projectId = $currentProjectId.get()
  if (!projectId) {
    $graphRecords.set({ data: [], loading: false, total: 0, loadedBatches: 0, hasMore: false })
    return
  }

  const skip = currentState.loadedBatches * GRAPH_BATCH_SIZE
  $graphRecords.set({ ...currentState, loading: true })

  try {
    const response = await fetchGraphRecords(skip, GRAPH_BATCH_SIZE)
    if (!response) return

    const newData = [...currentState.data, ...response.data]
    const newBatchCount = currentState.loadedBatches + 1
    const hasMore = response.hasMore && newBatchCount < GRAPH_MAX_BATCHES

    $graphRecords.set({
      data: newData,
      total: response.total,
      loading: false,
      loadedBatches: newBatchCount,
      hasMore
    })

    // Auto-load next batch if there's more data and we haven't hit the limit
    if (hasMore && newData.length < GRAPH_MAX_RECORDS) {
      setTimeout(() => ($graphRecords as any).loadNextBatch(), 100)
    }
  } catch (error) {
    if (error instanceof Error) {
      $graphRecords.set({
        ...currentState,
        loading: false,
        error: error.message
      })
    }
  }
}

// Sequential batch loading for relations
// @ts-ignore
$graphRelations.loadNextBatch = async () => {
  const currentState = $graphRelations.get()
  if (currentState.loading || !currentState.hasMore || currentState.loadedBatches >= GRAPH_MAX_BATCHES) {
    return
  }

  const projectId = $currentProjectId.get()
  if (!projectId) {
    $graphRelations.set({ data: [], loading: false, total: 0, loadedBatches: 0, hasMore: false })
    return
  }

  const skip = currentState.loadedBatches * GRAPH_BATCH_SIZE
  $graphRelations.set({ ...currentState, loading: true })

  try {
    const response = await fetchGraphRelations(skip, GRAPH_BATCH_SIZE)
    if (!response) return

    const newData = [...currentState.data, ...response.data]
    const newBatchCount = currentState.loadedBatches + 1
    const hasMore = response.hasMore && newBatchCount < GRAPH_MAX_BATCHES

    $graphRelations.set({
      data: newData,
      total: response.total,
      loading: false,
      loadedBatches: newBatchCount,
      hasMore
    })

    // Auto-load next batch if there's more data and we haven't hit the limit
    if (hasMore && newData.length < GRAPH_MAX_RECORDS) {
      setTimeout(() => ($graphRelations as any).loadNextBatch(), 100)
    }
  } catch (error) {
    if (error instanceof Error) {
      $graphRelations.set({
        ...currentState,
        loading: false,
        error: error.message
      })
    }
  }
}

// Reset function for both stores
// @ts-ignore
$graphRecords.reset = () => {
  $graphRecords.set({
    data: [],
    loading: false,
    error: undefined,
    total: undefined,
    loadedBatches: 0,
    hasMore: true
  })
}

// @ts-ignore
$graphRelations.reset = () => {
  $graphRelations.set({
    data: [],
    loading: false,
    error: undefined,
    total: undefined,
    loadedBatches: 0,
    hasMore: true
  })
}

// Start loading function - resets stores and begins batch loading
const startLoadingRecords = () => {
  ;($graphRecords as any).reset()
  setTimeout(() => ($graphRecords as any).loadNextBatch(), 0)
}

const startLoadingRelations = () => {
  ;($graphRelations as any).reset()
  setTimeout(() => ($graphRelations as any).loadNextBatch(), 0)
}

// Reset on dependency changes
for (const dep of [
  $currentProjectFilters,
  $recordsOrderBy,
  $activeLabels,
  $combineFilters,
  $currentProjectId
]) {
  onNotify(dep as any, () => {
    queueMicrotask(() => {
      startLoadingRecords()
      startLoadingRelations()
    })
  })
}

// Initialize loading when stores are first mounted
onMount($graphRecords, () => {
  const projectId = $currentProjectId.get()
  if (projectId) {
    startLoadingRecords()
  }
  return () => {}
})

onMount($graphRelations, () => {
  const projectId = $currentProjectId.get()
  if (projectId) {
    startLoadingRelations()
  }
  return () => {}
})

const normalizeRecord = (r: any): GraphNode | null => {
  const id = r.__id
  if (!id) return null
  const label = r.__label
  const pt = r.__proptypes
  return {
    id,
    type: 'record',
    label,
    name: (r.data?.name || r.data?.title || label) as string,
    proptypes: pt,
    properties: r,
    val: 4
  }
}

export const useGraphData = () => {
  // Use graph-specific stores that can load up to 10k records
  const recordsStore = useStore($graphRecords)
  const relationsStore = useStore($graphRelations)
  const fieldsStore = useStore($currentProjectFields)

  const [showProperties, setShowProperties] = useState(false)
  const [dataVersion, setDataVersion] = useState(0)

  // Transform store data into refs for compatibility with existing graph logic
  const foundRelationships = useRef<Map<string, Relation>>(new Map())
  const foundRecords = useRef<Map<string, any>>(new Map())
  const foundProperties = useRef<Map<string, any>>(new Map())

  const [graphData, setGraphData] = useState<{ nodes: GraphNode[]; links: GraphLink[] }>({
    nodes: [],
    links: []
  })

  // Build graph data when underlying refs change
  useEffect(() => {
    const nodes = Array.from(foundRecords.current.values())
      .filter((r: any) => !!r.__id)
      .map(normalizeRecord)
      .filter((n: GraphNode | null): n is GraphNode => n !== null)

    const properties: GraphNode[] = Array.from(foundProperties.current.values()).map((p: any) => ({
      id: p.id || p.__id,
      type: 'property',
      name: p.name,
      propertyType: p.type,
      properties: p,
      val: 1.5
    }))

    const recordIds = new Set(nodes.map((n) => n.id))
    const links: GraphLink[] = Array.from(foundRelationships.current.values())
      .filter((rel: any) => recordIds.has(rel.sourceId) && recordIds.has(rel.targetId))
      .map((rel: any) => ({
        source: rel.sourceId,
        target: rel.targetId,
        type: rel.type || 'rel'
      }))

    const propertyLinks: GraphLink[] = []
    properties.forEach((prop) => {
      nodes.forEach((node) => {
        if (node.proptypes && prop.name && node.proptypes[prop.name] === prop.propertyType) {
          propertyLinks.push({ source: prop.id, target: node.id, type: 'value' })
        }
      })
    })

    const finalNodes = showProperties ? [...nodes, ...properties] : nodes
    const finalLinks = showProperties ? [...links, ...propertyLinks] : links

    // cross-link for highlighting
    const nodeMap = new Map(finalNodes.map((n) => [n.id, n]))
    finalLinks.forEach((l) => {
      const a = typeof l.source === 'string' ? nodeMap.get(l.source) : l.source
      const b = typeof l.target === 'string' ? nodeMap.get(l.target) : l.target
      if (!a || !b) return
      ;(a.neighbors || (a.neighbors = [])).push(b)
      ;(b.neighbors || (b.neighbors = [])).push(a)
      ;(a.links || (a.links = [])).push(l)
      ;(b.links || (b.links = [])).push(l)
    })

    setGraphData({ nodes: finalNodes, links: finalLinks })
  }, [dataVersion, showProperties])

  // Update refs when store data changes and increment version to trigger graph rebuild
  useEffect(() => {
    // Clear and rebuild records map
    foundRecords.current.clear()
    if (recordsStore.data && recordsStore.data.length > 0) {
      recordsStore.data.forEach((record: any) => {
        // Handle both DBRecordInstance and raw record formats
        const recordData = record.data || record
        if (recordData.__id) {
          foundRecords.current.set(recordData.__id, recordData)
        }
      })
    }
    setDataVersion((v) => v + 1)
  }, [recordsStore.data])

  useEffect(() => {
    // Clear and rebuild relationships map
    foundRelationships.current.clear()
    if (relationsStore.data) {
      relationsStore.data.forEach((relation: any) => {
        const key = `${relation.sourceId}|${relation.targetId}|${relation.type}`
        foundRelationships.current.set(key, relation)
      })
    }
    setDataVersion((v) => v + 1)
  }, [relationsStore.data])

  useEffect(() => {
    // Clear and rebuild properties map
    foundProperties.current.clear()
    if (fieldsStore.data) {
      fieldsStore.data.forEach((field: Property) => {
        const key = field.id
        if (key) {
          foundProperties.current.set(key, field)
        }
      })
    }
    setDataVersion((v) => v + 1)
  }, [fieldsStore.data])

  // Compute loading and error state from all stores
  // Show loading if any store is actively loading OR if there's more data to load automatically
  const isLoadingRecords =
    recordsStore.loading || (recordsStore.hasMore && recordsStore.loadedBatches < GRAPH_MAX_BATCHES)
  const isLoadingRelations =
    relationsStore.loading || (relationsStore.hasMore && relationsStore.loadedBatches < GRAPH_MAX_BATCHES)
  const loading = isLoadingRecords || isLoadingRelations || fieldsStore.loading
  const error = recordsStore.error || relationsStore.error || fieldsStore.error

  // Fetch functions that trigger store loading
  const fetchData = async (reset = false) => {
    // Start loading both records and relations from scratch
    startLoadingRecords()
    startLoadingRelations()
    // Also refresh fields
    ;($currentProjectFields as any).refetch?.()
  }

  const fetchRecords = async (reset = false) => {
    startLoadingRecords()
  }

  const fetchRelationships = async (reset = false) => {
    startLoadingRelations()
  }

  const fetchProperties = async (reset = false) => {
    ;($currentProjectFields as any).refetch?.()
  }

  // Manual batch loading functions
  const loadMoreRecords = async () => {
    if (recordsStore.hasMore && !recordsStore.loading) {
      ;($graphRecords as any).loadNextBatch()
    }
  }

  const loadMoreRelationships = async () => {
    if (relationsStore.hasMore && !relationsStore.loading) {
      ;($graphRelations as any).loadNextBatch()
    }
  }

  // Computed hasMore flags
  const hasMoreRecords = recordsStore.hasMore
  const hasMoreRelationships = relationsStore.hasMore

  useEffect(() => {
    // initial load - start loading graph data when component mounts
    const projectId = $currentProjectId.get()
    if (projectId) {
      fetchData(true)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return {
    loading,
    graphData,
    setLoading: () => {}, // no-op since loading is computed from stores
    error,
    foundRecords,
    foundRelationships,
    foundProperties,
    hasMoreRecords,
    hasMoreRelationships,
    showProperties,
    setShowProperties,
    fetchData,
    fetchRecords,
    fetchRelationships,
    fetchProperties,
    loadMoreRecords,
    loadMoreRelationships,
    dataVersion,
    // Additional graph stats
    totalRecordsLoaded: recordsStore.data.length,
    totalRelationsLoaded: relationsStore.data.length,
    recordsBatchesLoaded: recordsStore.loadedBatches,
    relationsBatchesLoaded: relationsStore.loadedBatches,
    maxPossibleRecords: GRAPH_MAX_RECORDS,
    // Detailed loading states
    isLoadingRecords,
    isLoadingRelations,
    recordsTotal: recordsStore.total,
    relationsTotal: relationsStore.total,
    recordsLoadingProgress:
      recordsStore.total ?
        Math.min(100, (recordsStore.data.length / Math.min(recordsStore.total, GRAPH_MAX_RECORDS)) * 100)
      : 0,
    relationsLoadingProgress:
      relationsStore.total ?
        Math.min(100, (relationsStore.data.length / Math.min(relationsStore.total, GRAPH_MAX_RECORDS)) * 100)
      : 0
  }
}
