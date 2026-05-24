import type { FC } from 'react'
import React, { useRef, useCallback, useState, useEffect, useMemo } from 'react'

import { useStore } from '@nanostores/react'
import { Layers, RotateCcw, ScanSearch } from 'lucide-react'
import { ForceGraph2D, ForceGraph3D } from 'react-force-graph'
import SpriteText from 'three-spritetext'
import * as THREE from 'three'

import {
  useFilteredRecordRelationsQuery,
  useFilteredRecordsQuery,
  useProjectFieldsQuery,
  useProjectIndexesQuery,
  useProjectLabelsQuery
} from '~/features/projects/hooks/useProjectQueries'
import { Button } from '~/elements/Button'
import { Tooltip } from '~/elements/Tooltip'
import { Checkbox } from '~/elements/Checkbox'
import { Tab, Tabs, TabsList } from '~/elements/Tabs'
import { IconButton } from '~/elements/IconButton'
import { Menu, MenuTitle } from '~/elements/Menu'
import { $sheetProperty, $sheetRecordId, type PropertySheetData } from '~/features/projects/stores/id.ts'
import { getLabelColor } from '~/features/labels'
import type { DBRecord, DBRecordInstance } from '@rushdb/javascript-sdk'
import { type Relation } from '@rushdb/javascript-sdk'

export type GraphMode = '2d' | '3d'
type GraphNodeKind = 'record' | 'property'
type GraphLinkKind = 'record-relation' | 'property-value'

export type GraphNode = {
  id: string
  kind: GraphNodeKind
  label: string
  color: string
  recordId?: string
  propertyName?: string
  propertyType?: string
  propertyLabels?: string[]
  vectorIndexed?: boolean
  connectedRecordIds?: string[]
}

export type GraphLink = {
  id: string
  kind: GraphLinkKind
  source: string
  target: string
  relationType?: string
}

type GraphOutput = {
  nodes: GraphNode[]
  links: GraphLink[]
}

type HoverHighlights = {
  nodeIds: Set<string>
  linkIds: Set<string>
}

function LayerToggle({
  checked,
  description,
  label,
  onCheckedChange
}: {
  checked: boolean
  description: string
  label: string
  onCheckedChange: () => void
}) {
  return (
    <label className="flex cursor-pointer items-start gap-3">
      <Checkbox checked={checked} className="mt-1" onCheckedChange={onCheckedChange} />
      <span className="flex min-w-0 flex-col gap-0.5">
        <span className="text-content text-sm font-medium">{label}</span>
        <span className="text-content2 text-sm leading-snug">{description}</span>
      </span>
    </label>
  )
}

function getGraphRefId(ref: unknown): string | undefined {
  if (typeof ref === 'string') return ref
  if (ref && typeof ref === 'object' && 'id' in (ref as Record<string, unknown>)) {
    const value = (ref as { id?: unknown }).id
    return typeof value === 'string' ? value : undefined
  }
  return undefined
}

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
  const linkMap = new Map<string, GraphLink>()

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
          color: '#accd17',
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

function drawSquare(ctx: CanvasRenderingContext2D, x: number, y: number, size: number) {
  const half = size / 2
  ctx.beginPath()
  ctx.rect(x - half, y - half, size, size)
  ctx.closePath()
}

function withOpacity(color: string, opacity: number): string {
  if (color.startsWith('rgba(')) {
    return color.replace(/rgba\((\d+),\s*(\d+),\s*(\d+),\s*[^)]+\)/, `rgba($1,$2,$3,${opacity})`)
  }

  if (color.startsWith('rgb(')) {
    const inside = color.slice(4, -1)
    return `rgba(${inside}, ${opacity})`
  }

  if (color.startsWith('#')) {
    let hex = color.slice(1)
    if (hex.length === 3) {
      hex = hex
        .split('')
        .map((ch) => `${ch}${ch}`)
        .join('')
    }

    if (hex.length === 6) {
      const r = Number.parseInt(hex.slice(0, 2), 16)
      const g = Number.parseInt(hex.slice(2, 4), 16)
      const b = Number.parseInt(hex.slice(4, 6), 16)
      return `rgba(${r}, ${g}, ${b}, ${opacity})`
    }
  }

  return color
}

function getNodeHoverLabel(node: GraphNode): string {
  const shellStyle =
    'background: rgba(0, 0, 0, 0.5); color: #fff; border: 1px solid rgba(255, 255, 255, 0.2); border-radius: 8px; padding: 8px 10px; line-height: 1.35; font-size: 12px; backdrop-filter: blur(2px); max-width: 320px;'

  if (node.kind === 'record') {
    return `<div style="${shellStyle}"><div><b>Record</b>: ${node.label}</div><div>ID: ${node.recordId ?? 'N/A'}</div></div>`
  }

  const vectorState = node.vectorIndexed ? 'yes' : 'no'
  return `<div style="${shellStyle}"><div><b>Property</b>: ${node.propertyName ?? node.label}</div><div>Type: ${node.propertyType ?? 'string'}</div><div>Vector indexed: ${vectorState}</div></div>`
}

const HEADER_HEIGHT = 182
const FOOTER_HEIGHT = 61

function renderLinkLabel2D({
  ctx,
  globalScale,
  highlighted,
  labelScale = 1,
  link,
  visible = highlighted
}: {
  ctx: CanvasRenderingContext2D
  globalScale: number
  highlighted: boolean
  labelScale?: number
  link: GraphLink
  visible?: boolean
}) {
  if (!visible || !link.relationType || link.kind !== 'record-relation') {
    return
  }

  const source = link.source as unknown as { x?: number; y?: number }
  const target = link.target as unknown as { x?: number; y?: number }
  const sourceX = source.x ?? 0
  const sourceY = source.y ?? 0
  const targetX = target.x ?? 0
  const targetY = target.y ?? 0
  const x = (sourceX + targetX) / 2
  const y = (sourceY + targetY) / 2
  const fontSize = Math.min(Math.max((8 * labelScale) / globalScale, 2.5), 11 * labelScale)

  ctx.save()
  ctx.font = `${fontSize}px monospace`
  ctx.fillStyle = highlighted ? '#d9e2ec' : '#9aa7b8'
  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
  ctx.fillText(link.relationType, x, y)
  ctx.restore()
}

export const GraphView: FC = () => {
  const fgRef = useRef<any>(null)
  const pinned2DPositionsRef = useRef<Record<string, { x: number; y: number }>>({})
  const pinned3DPositionsRef = useRef<Record<string, { x: number; y: number; z: number }>>({})
  const draggingNodeIdRef = useRef<string | undefined>(undefined)

  const { data: relationsResult } = useFilteredRecordRelationsQuery()
  const { data: recordsResult } = useFilteredRecordsQuery()
  const { data: labels } = useProjectLabelsQuery()
  const { data: indexes } = useProjectIndexesQuery()
  const { data: fields } = useProjectFieldsQuery()
  const relations = relationsResult?.data
  const records = recordsResult?.data

  const [graphMode, setGraphMode] = useState<GraphMode>('2d')
  const [showProperties, setShowProperties] = useState(true)
  const [showPropertyLinks, setShowPropertyLinks] = useState(true)
  const [showRecordLinks, setShowRecordLinks] = useState(true)
  const [showRecordLabels, setShowRecordLabels] = useState(true)
  const [showRelationshipTypes, setShowRelationshipTypes] = useState(true)
  const [hoveredNodeId, setHoveredNodeId] = useState<string | undefined>(undefined)
  const [hoveredLinkId, setHoveredLinkId] = useState<string | undefined>(undefined)

  const selectedProperty = useStore($sheetProperty)

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

  const visibleGraphData = useMemo(() => {
    const visibleNodes = rawGraphData.nodes.filter((node) => {
      if (!showProperties && node.kind === 'property') return false
      return true
    })
    const visibleNodeIds = new Set(visibleNodes.map((node) => node.id))

    const visibleLinks = rawGraphData.links
      .map((link) => {
        const sourceId = getGraphRefId((link as unknown as { source?: unknown }).source)
        const targetId = getGraphRefId((link as unknown as { target?: unknown }).target)
        if (!sourceId || !targetId) return undefined

        return {
          ...link,
          source: sourceId,
          target: targetId
        } satisfies GraphLink
      })
      .filter((link): link is GraphLink => !!link)
      .filter((link) => {
        if (!visibleNodeIds.has(link.source) || !visibleNodeIds.has(link.target)) return false
        if (!showRecordLinks && link.kind === 'record-relation') return false
        if (!showPropertyLinks && link.kind === 'property-value') return false
        return true
      })

    return {
      nodes: visibleNodes,
      links: visibleLinks
    }
  }, [rawGraphData.links, rawGraphData.nodes, showProperties, showPropertyLinks, showRecordLinks])

  const hoverHighlights = useMemo<HoverHighlights>(() => {
    const highlightedNodeIds = new Set<string>()
    const highlightedLinkIds = new Set<string>()

    if (!hoveredNodeId && !hoveredLinkId) {
      return {
        nodeIds: highlightedNodeIds,
        linkIds: highlightedLinkIds
      }
    }

    const linksByNodeId = new Map<string, GraphLink[]>()
    visibleGraphData.links.forEach((link) => {
      const source = getGraphRefId((link as unknown as { source?: unknown }).source)
      const target = getGraphRefId((link as unknown as { target?: unknown }).target)
      if (!source || !target) return

      const sourceLinks = linksByNodeId.get(source) ?? []
      sourceLinks.push(link)
      linksByNodeId.set(source, sourceLinks)

      const targetLinks = linksByNodeId.get(target) ?? []
      targetLinks.push(link)
      linksByNodeId.set(target, targetLinks)
    })

    if (hoveredNodeId) {
      highlightedNodeIds.add(hoveredNodeId)

      const connected = linksByNodeId.get(hoveredNodeId) ?? []
      connected.forEach((link) => {
        highlightedLinkIds.add(link.id)
        const source = getGraphRefId((link as unknown as { source?: unknown }).source)
        const target = getGraphRefId((link as unknown as { target?: unknown }).target)
        if (source) highlightedNodeIds.add(source)
        if (target) highlightedNodeIds.add(target)
      })
    }

    if (hoveredLinkId) {
      const hoveredLink = visibleGraphData.links.find((link) => link.id === hoveredLinkId)
      if (hoveredLink) {
        highlightedLinkIds.add(hoveredLink.id)

        const source = getGraphRefId((hoveredLink as unknown as { source?: unknown }).source)
        const target = getGraphRefId((hoveredLink as unknown as { target?: unknown }).target)

        if (source) {
          highlightedNodeIds.add(source)
          ;(linksByNodeId.get(source) ?? []).forEach((link) => highlightedLinkIds.add(link.id))
        }
        if (target) {
          highlightedNodeIds.add(target)
          ;(linksByNodeId.get(target) ?? []).forEach((link) => highlightedLinkIds.add(link.id))
        }

        highlightedLinkIds.forEach((id) => {
          const link = visibleGraphData.links.find((item) => item.id === id)
          if (!link) return
          const src = getGraphRefId((link as unknown as { source?: unknown }).source)
          const dst = getGraphRefId((link as unknown as { target?: unknown }).target)
          if (src) highlightedNodeIds.add(src)
          if (dst) highlightedNodeIds.add(dst)
        })
      }
    }

    return {
      nodeIds: highlightedNodeIds,
      linkIds: highlightedLinkIds
    }
  }, [hoveredLinkId, hoveredNodeId, visibleGraphData.links])

  const hasHoverSelection = hoveredNodeId !== undefined || hoveredLinkId !== undefined

  const isNodeHighlighted = useCallback(
    (nodeId: string) => !hasHoverSelection || hoverHighlights.nodeIds.has(nodeId),
    [hasHoverSelection, hoverHighlights.nodeIds]
  )

  const isLinkHighlighted = useCallback(
    (linkId: string) => !hasHoverSelection || hoverHighlights.linkIds.has(linkId),
    [hasHoverSelection, hoverHighlights.linkIds]
  )

  useEffect(() => {
    if (draggingNodeIdRef.current) {
      return
    }

    if (graphMode === '2d') {
      // Apply saved 2D pinned positions when available.
      visibleGraphData.nodes.forEach((node) => {
        const pinned = pinned2DPositionsRef.current[node.id]
        const mutable = node as any
        if (pinned) {
          mutable.fx = pinned.x
          mutable.fy = pinned.y
        } else {
          mutable.fx = undefined
          mutable.fy = undefined
        }
        mutable.fz = undefined
      })
      return
    }

    // Re-apply saved 3D pinned positions after data/mode changes.
    visibleGraphData.nodes.forEach((node) => {
      const pinned = pinned3DPositionsRef.current[node.id]
      const mutable = node as any
      if (pinned) {
        mutable.fx = pinned.x
        mutable.fy = pinned.y
        mutable.fz = pinned.z
      } else {
        mutable.fx = undefined
        mutable.fy = undefined
        mutable.fz = undefined
      }
    })
  }, [graphMode, visibleGraphData])

  const focusNode = useCallback(
    (node: any) => {
      // ForceGraph3D supports cameraPosition, while ForceGraph2D uses centerAt/zoom.
      if (graphMode === '2d') {
        fgRef.current?.centerAt?.(node.x ?? 0, node.y ?? 0, 700)
        fgRef.current?.zoom?.(3.2, 700)
        return
      }

      const distance = 260
      const safeDistance = Math.max(1, Math.hypot(node.x ?? 0, node.y ?? 0, node.z ?? 0))
      const distRatio = 1 + distance / safeDistance

      fgRef.current?.cameraPosition?.(
        { x: (node.x ?? 0) * distRatio, y: (node.y ?? 0) * distRatio, z: (node.z ?? 0) * distRatio },
        node,
        1000
      )
    },
    [graphMode]
  )

  const handleClick = useCallback(
    (node: GraphNode, event?: MouseEvent) => {
      focusNode(node)

      if (node.kind === 'record' && node.recordId) {
        $sheetProperty.set(undefined)
        $sheetRecordId.set(node.recordId)
        return
      }

      if (node.kind === 'property') {
        $sheetRecordId.set(undefined)
        const name = node.propertyName ?? node.label
        const type = node.propertyType ?? 'string'
        const field = fields?.find((f) => f.name === name && f.type === type)
        $sheetProperty.set({
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
    [focusNode, fields]
  )

  const handleLinkClick = useCallback((link: any) => {
    const distance = 70

    const sx = link.source.x ?? 0
    const sy = link.source.y ?? 0
    const sz = link.source.z ?? 0

    const tx = link.target.x ?? 0
    const ty = link.target.y ?? 0
    const tz = link.target.z ?? 0

    const dx = sx - tx
    const dy = sy - ty
    const dz = sz - tz

    const rdx = dz
    const rdy = dy
    const rdz = -dx

    const dist = Math.max(1, Math.hypot(rdx, rdy, rdz))

    const distRatio = 1 + distance / dist

    const mx = (sx + tx) / 2
    const my = (sy + ty) / 2
    const mz = (sz + tz) / 2

    const cx = mx + rdx * distRatio
    const cy = my + rdy * distRatio
    const cz = mz + rdz * distRatio

    fgRef.current?.cameraPosition({ x: cx, y: cy, z: cz }, { x: mx, y: my, z: mz }, 1000)
  }, [])

  const [canvasSize, setCanvasSize] = useState({
    width: typeof window === 'undefined' ? 1200 : window.innerWidth - 16,
    height: typeof window === 'undefined' ? 680 : window.innerHeight - HEADER_HEIGHT - FOOTER_HEIGHT
  })

  useEffect(() => {
    const resize = () => {
      setCanvasSize({
        width: window.innerWidth - 16,
        height: window.innerHeight - HEADER_HEIGHT - FOOTER_HEIGHT
      })
    }
    window.addEventListener('resize', resize)

    return () => {
      window.removeEventListener('resize', resize)
    }
  }, [])

  const fitGraph = useCallback(() => {
    if (graphMode !== '3d') {
      fgRef.current?.zoomToFit?.(400, 30)
      return
    }

    const positionedNodes = visibleGraphData.nodes
      .map((node) => {
        const mutable = node as any
        return {
          x: Number(mutable.x),
          y: Number(mutable.y),
          z: Number(mutable.z)
        }
      })
      .filter((node) => Number.isFinite(node.x) && Number.isFinite(node.y) && Number.isFinite(node.z))

    if (!positionedNodes.length) {
      fgRef.current?.zoomToFit?.(400, 30)
      return
    }

    const center = positionedNodes.reduce(
      (acc, node) => ({
        x: acc.x + node.x / positionedNodes.length,
        y: acc.y + node.y / positionedNodes.length,
        z: acc.z + node.z / positionedNodes.length
      }),
      { x: 0, y: 0, z: 0 }
    )
    const distances = positionedNodes
      .map((node) => Math.hypot(node.x - center.x, node.y - center.y, node.z - center.z))
      .sort((a, b) => a - b)
    const mainClusterRadius =
      distances[Math.min(distances.length - 1, Math.floor(distances.length * 0.9))] ?? 80
    const distance = Math.min(900, Math.max(120, mainClusterRadius * 2.4))
    const camera = fgRef.current?.camera?.()
    const direction = new THREE.Vector3(
      camera?.position?.x ?? 0,
      camera?.position?.y ?? 0,
      camera?.position?.z ?? 1
    )
      .sub(new THREE.Vector3(center.x, center.y, center.z))
      .normalize()

    if (!Number.isFinite(direction.x) || !Number.isFinite(direction.y) || !Number.isFinite(direction.z)) {
      direction.set(0, 0, 1)
    }

    fgRef.current?.cameraPosition?.(
      {
        x: center.x + direction.x * distance,
        y: center.y + direction.y * distance,
        z: center.z + direction.z * distance
      },
      center,
      500
    )
  }, [graphMode, visibleGraphData.nodes])

  const reheatSimulation = useCallback(() => {
    // Relayout should start from a clean simulation state with no pinned nodes.
    pinned2DPositionsRef.current = {}
    pinned3DPositionsRef.current = {}

    visibleGraphData.nodes.forEach((node) => {
      const mutable = node as any
      mutable.fx = undefined
      mutable.fy = undefined
      mutable.fz = undefined
    })

    fgRef.current?.d3ReheatSimulation?.()
  }, [visibleGraphData.nodes])

  const createNodeObject3D = useCallback(
    (node: GraphNode) => {
      const size = 9

      if (node.kind === 'property') {
        return new THREE.Mesh(
          new THREE.BoxGeometry(size, size, size),
          new THREE.MeshLambertMaterial({ color: node.color })
        )
      }

      const group = new THREE.Group()

      const sphere = new THREE.Mesh(
        new THREE.SphereGeometry(size / 2, 20, 20),
        new THREE.MeshLambertMaterial({ color: node.color })
      )
      group.add(sphere)

      if (showRecordLabels) {
        // Keep record labels visible in 3D with a dimmed color to reduce visual noise.
        const label = new SpriteText(node.label)
        label.color = '#9aa7b8'
        label.textHeight = 1.45
        label.position.set(0, -6.3, 0)
        group.add(label)
      }

      return group
    },
    [showRecordLabels]
  )

  const createLinkObject3D = useCallback(
    (link: GraphLink) => {
      if (!showRelationshipTypes || !link.relationType || link.kind !== 'record-relation') {
        return undefined
      }

      const label = new SpriteText(link.relationType)
      const highlighted = !hasHoverSelection || hoveredLinkId === link.id
      label.color = highlighted ? '#d9e2ec' : '#9aa7b8'
      label.textHeight = highlighted ? 1.25 : 1
      return label
    },
    [hasHoverSelection, hoveredLinkId, showRelationshipTypes]
  )

  const updateLinkObjectPosition3D = useCallback(
    (object: THREE.Object3D | undefined, { start, end }: any) => {
      if (!object) {
        return false
      }

      object.position.x = start.x + (end.x - start.x) * 0.52
      object.position.y = start.y + (end.y - start.y) * 0.52
      object.position.z = start.z + (end.z - start.z) * 0.52
      return true
    },
    []
  )

  const renderNode2D = useCallback(
    (node: GraphNode, ctx: CanvasRenderingContext2D, globalScale: number) => {
      const x = (node as any).x ?? 0
      const y = (node as any).y ?? 0
      const radius = 5
      const highlighted = isNodeHighlighted(node.id)
      const dimmed = hasHoverSelection && !highlighted

      ctx.save()
      ctx.fillStyle = dimmed ? withOpacity(node.color, 0.2) : node.color

      if (node.kind === 'property') {
        drawSquare(ctx, x, y, radius * 2)
        ctx.fill()
      } else {
        ctx.beginPath()
        ctx.arc(x, y, radius, 0, Math.PI * 2)
        ctx.closePath()
        ctx.fill()
      }

      // vector badge
      if (node.kind === 'property' && node.vectorIndexed) {
        ctx.beginPath()
        ctx.fillStyle = dimmed ? withOpacity('#10b981', 0.2) : '#10b981'
        ctx.arc(x - 4.5, y - 4.5, 1.6, 0, Math.PI * 2)
        ctx.fill()
      }

      if (node.kind === 'record' && showRecordLabels) {
        const fontSize = Math.max(8 / globalScale, 2.5)
        ctx.font = `${fontSize}px monospace`
        ctx.fillStyle = dimmed ? withOpacity('#9aa7b8', 0.2) : '#9aa7b8'
        ctx.textAlign = 'center'
        ctx.fillText(node.label, x, y + 9)
      }
      ctx.restore()
    },
    [hasHoverSelection, isNodeHighlighted, showRecordLabels]
  )

  const renderNodePointerArea2D = useCallback(
    (node: GraphNode, color: string, ctx: CanvasRenderingContext2D) => {
      const x = (node as any).x ?? 0
      const y = (node as any).y ?? 0
      const radius = 6

      ctx.fillStyle = color
      if (node.kind === 'property') {
        drawSquare(ctx, x, y, radius * 2)
        ctx.fill()
        return
      }

      ctx.beginPath()
      ctx.arc(x, y, radius, 0, Math.PI * 2)
      ctx.closePath()
      ctx.fill()
    },
    []
  )

  const renderLink2D = useCallback(
    (link: GraphLink, ctx: CanvasRenderingContext2D, globalScale: number) => {
      renderLinkLabel2D({
        ctx,
        globalScale,
        highlighted: isLinkHighlighted(link.id),
        link,
        visible: showRelationshipTypes && isLinkHighlighted(link.id)
      })
    },
    [isLinkHighlighted, showRelationshipTypes]
  )

  const renderCommonGraphControls = (
    <>
      <div className="absolute left-4 top-4 z-20 flex flex-col gap-2">
        <Menu
          align="start"
          className="min-w-[240px] p-2"
          modal={false}
          trigger={
            <IconButton
              aria-label="Graph layers"
              className="bg-fill/90 shadow-xl backdrop-blur-sm"
              size="small"
              variant="outline"
            >
              <Layers />
            </IconButton>
          }
        >
          <MenuTitle className="px-2 pb-1 pt-1 uppercase tracking-wide">Graph Layers</MenuTitle>
          <div className="flex flex-col gap-3 px-2 pb-2" onClick={(event) => event.stopPropagation()}>
            <div className="flex flex-col gap-2">
              <div className="text-content3 text-sm font-semibold uppercase">Nodes</div>
              <LayerToggle
                checked={showProperties}
                description="Show shared property nodes around records."
                label="Property nodes"
                onCheckedChange={() => setShowProperties((v) => !v)}
              />
            </div>

            <div className="flex flex-col gap-2">
              <div className="text-content3 text-sm font-semibold uppercase">Links</div>
              <LayerToggle
                checked={showPropertyLinks}
                description="Show links between properties and records."
                label="Property links"
                onCheckedChange={() => setShowPropertyLinks((v) => !v)}
              />
              <LayerToggle
                checked={showRecordLinks}
                description="Show relationships between records."
                label="Record relationships"
                onCheckedChange={() => setShowRecordLinks((v) => !v)}
              />
            </div>

            <div className="flex flex-col gap-2">
              <div className="text-content3 text-sm font-semibold uppercase">Labels</div>
              <LayerToggle
                checked={showRecordLabels}
                description="Show label names under record nodes."
                label="Record labels"
                onCheckedChange={() => setShowRecordLabels((v) => !v)}
              />
              <LayerToggle
                checked={showRelationshipTypes}
                description="Show relationship type names on record links."
                label="Relationship types"
                onCheckedChange={() => setShowRelationshipTypes((v) => !v)}
              />
            </div>
          </div>
        </Menu>
        <IconButton
          aria-label="Fit graph"
          className="bg-fill/90 shadow-xl backdrop-blur-sm"
          onClick={fitGraph}
          size="small"
          variant="outline"
        >
          <ScanSearch />
        </IconButton>
        <IconButton
          aria-label="Relayout graph"
          className="bg-fill/90 shadow-xl backdrop-blur-sm"
          onClick={reheatSimulation}
          size="small"
          variant="outline"
        >
          <RotateCcw />
        </IconButton>
      </div>
    </>
  )

  return (
    <div className="graph-view relative">
      <style>{`
        .graph-view .graph-tooltip,
        .graph-tooltip {
          background: transparent !important;
          color: #fff !important;
          border: none !important;
          border-radius: 0;
          padding: 0 !important;
          line-height: 1.35;
          font-size: 12px;
          backdrop-filter: none;
          max-width: 320px;
          box-shadow: none !important;
        }
      `}</style>
      {renderCommonGraphControls}

      <Tabs
        className="absolute right-4 top-4 z-20"
        onValueChange={(value) => setGraphMode(value as GraphMode)}
        value={graphMode}
      >
        <TabsList>
          <Tab value="2d">2D</Tab>
          <Tab value="3d">3D</Tab>
        </TabsList>
      </Tabs>

      {selectedProperty && (
        <div className="bg-fill/90 border-content/30 absolute right-4 top-20 z-20 max-w-xs rounded-xl border p-3 shadow-xl backdrop-blur-sm">
          <div className="text-xs font-semibold uppercase tracking-wide">Selected Property</div>
          <div className="mt-1 text-sm font-medium">{selectedProperty.name}</div>
          <div className="text-content-secondary text-xs">Alt + click node to hide it</div>
        </div>
      )}

      {graphMode === '3d' ?
        <ForceGraph3D
          ref={fgRef}
          backgroundColor={'#222'}
          showNavInfo={false}
          graphData={visibleGraphData}
          linkWidth={(link: GraphLink) => {
            return 0
          }}
          linkOpacity={1}
          linkColor={(link: GraphLink) => {
            const highlighted = isLinkHighlighted(link.id)
            if (highlighted) {
              return link.kind === 'record-relation' ? '#d8e0e8' : '#86909b'
            }
            return link.kind === 'record-relation' ? '#5f6872' : '#3f4852'
          }}
          nodeOpacity={hasHoverSelection ? 0.24 : 1}
          nodeRelSize={4.5}
          nodeId={'id'}
          linkSource={'source'}
          linkTarget={'target'}
          nodeResolution={24}
          height={canvasSize.height}
          width={canvasSize.width}
          nodeColor={(node: GraphNode) =>
            !hasHoverSelection || isNodeHighlighted(node.id) ? node.color : withOpacity(node.color, 0.22)
          }
          linkDirectionalArrowRelPos={1}
          linkDirectionalArrowLength={(link: GraphLink) => (link.kind === 'record-relation' ? 1.15 : 0)}
          linkDirectionalArrowColor={(link: GraphLink) =>
            isLinkHighlighted(link.id) ? '#d8e0e8' : '#5f6872'
          }
          linkDirectionalArrowResolution={12}
          nodeThreeObject={createNodeObject3D as any}
          linkThreeObject={createLinkObject3D as any}
          linkThreeObjectExtend
          linkPositionUpdate={updateLinkObjectPosition3D as any}
          onLinkClick={handleLinkClick}
          onNodeHover={(node: GraphNode | null) => {
            if (draggingNodeIdRef.current) {
              return
            }
            setHoveredLinkId(undefined)
            setHoveredNodeId(node?.id)
          }}
          onLinkHover={(link: GraphLink | null) => {
            if (draggingNodeIdRef.current) {
              return
            }
            setHoveredNodeId(undefined)
            setHoveredLinkId(link?.id)
          }}
          onNodeDrag={(node: any) => {
            draggingNodeIdRef.current = String(node.id ?? '')
            node.fx = node.x
            node.fy = node.y
            node.fz = node.z
          }}
          onNodeDragEnd={(node: any) => {
            node.fx = node.x
            node.fy = node.y
            node.fz = node.z
            draggingNodeIdRef.current = undefined

            if (node?.id && Number.isFinite(node.x) && Number.isFinite(node.y) && Number.isFinite(node.z)) {
              pinned3DPositionsRef.current[String(node.id)] = {
                x: node.x,
                y: node.y,
                z: node.z
              }
            }
          }}
          nodeLabel={getNodeHoverLabel as any}
          onNodeClick={handleClick as any}
        />
      : <ForceGraph2D
          ref={fgRef}
          backgroundColor={'#222'}
          graphData={visibleGraphData}
          nodeId={'id'}
          linkSource={'source'}
          linkTarget={'target'}
          height={canvasSize.height}
          width={canvasSize.width}
          linkWidth={(link: GraphLink) => {
            const highlighted = isLinkHighlighted(link.id)
            const base = link.kind === 'record-relation' ? 1.6 : 0.8
            return highlighted ? base * 1.4 : base
          }}
          linkColor={(link: GraphLink) => {
            const highlighted = isLinkHighlighted(link.id)
            if (highlighted) {
              return link.kind === 'record-relation' ? 'rgba(210,220,230,0.75)' : 'rgba(210,220,230,0.38)'
            }
            return hasHoverSelection ? 'rgba(140,150,160,0.1)' : 'rgba(210,220,230,0.2)'
          }}
          nodeRelSize={5}
          linkDirectionalArrowRelPos={1}
          linkDirectionalArrowLength={(link: GraphLink) => (link.kind === 'record-relation' ? 2.2 : 0)}
          linkDirectionalArrowColor={(link: GraphLink) => {
            if (isLinkHighlighted(link.id)) {
              return 'rgb(210,220,230)'
            }
            return hasHoverSelection ? 'rgb(140,150,160)' : 'rgb(210,220,230)'
          }}
          linkCanvasObjectMode={() => 'after'}
          linkCanvasObject={renderLink2D as any}
          nodeCanvasObject={renderNode2D as any}
          nodePointerAreaPaint={renderNodePointerArea2D as any}
          cooldownTicks={120}
          onNodeHover={(node: GraphNode | null) => {
            if (draggingNodeIdRef.current) {
              return
            }
            setHoveredLinkId(undefined)
            setHoveredNodeId(node?.id)
          }}
          onLinkHover={(link: GraphLink | null) => {
            if (draggingNodeIdRef.current) {
              return
            }
            setHoveredNodeId(undefined)
            setHoveredLinkId(link?.id)
          }}
          onNodeDrag={(node: any) => {
            draggingNodeIdRef.current = String(node.id ?? '')
            node.fx = node.x
            node.fy = node.y
            node.fz = undefined
          }}
          onNodeDragEnd={(node: any) => {
            // 2D mode persists manually dragged positions.
            node.fx = node.x
            node.fy = node.y
            node.fz = undefined
            draggingNodeIdRef.current = undefined

            if (node?.id && Number.isFinite(node.x) && Number.isFinite(node.y)) {
              pinned2DPositionsRef.current[String(node.id)] = {
                x: node.x,
                y: node.y
              }
            }
          }}
          nodeLabel={getNodeHoverLabel as any}
          onNodeClick={handleClick as any}
        />
      }
    </div>
  )
}
