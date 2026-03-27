import React, { useRef, useCallback, FC, useState, useEffect, useMemo } from 'react'

import { useStore } from '@nanostores/react'
import { EyeOff, RotateCcw, ScanSearch, Square, Orbit } from 'lucide-react'
import { ForceGraph2D, ForceGraph3D } from 'react-force-graph'
import SpriteText from 'three-spritetext'
import * as THREE from 'three'

import {
  useFilteredRecordRelationsQuery,
  useFilteredRecordsQuery,
  useProjectIndexesQuery,
  useProjectLabelsQuery
} from '~/features/projects/hooks/useProjectQueries'
import { Button } from '~/elements/Button'
import { Tooltip } from '~/elements/Tooltip'
import { $sheetProperty, $sheetRecordId } from '~/features/projects/stores/id.ts'
import { getLabelColor } from '~/features/labels'
import { DBRecord, DBRecordInstance, type Relation } from '@rushdb/javascript-sdk'

type GraphMode = '2d' | '3d'
type GraphNodeKind = 'record' | 'property'
type GraphLinkKind = 'record-relation' | 'property-value'

type GraphNode = {
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

type GraphLink = {
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

export const GraphView: FC = () => {
  const fgRef = useRef<any>(null)
  const pinned2DPositionsRef = useRef<Record<string, { x: number; y: number }>>({})
  const pinned3DPositionsRef = useRef<Record<string, { x: number; y: number; z: number }>>({})

  const { data: relationsResult } = useFilteredRecordRelationsQuery()
  const { data: recordsResult } = useFilteredRecordsQuery()
  const { data: labels } = useProjectLabelsQuery()
  const { data: indexes } = useProjectIndexesQuery()
  const relations = relationsResult?.data
  const records = recordsResult?.data

  const [graphMode, setGraphMode] = useState<GraphMode>('3d')
  const [showProperties, setShowProperties] = useState(true)
  const [showPropertyLinks, setShowPropertyLinks] = useState(true)
  const [showRecordLinks, setShowRecordLinks] = useState(true)
  const [hiddenNodeIds, setHiddenNodeIds] = useState<string[]>([])
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
    const hiddenSet = new Set(hiddenNodeIds)
    const visibleNodes = rawGraphData.nodes.filter((node) => {
      if (hiddenSet.has(node.id)) return false
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
  }, [
    hiddenNodeIds,
    rawGraphData.links,
    rawGraphData.nodes,
    showProperties,
    showPropertyLinks,
    showRecordLinks
  ])

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

  useEffect(() => {
    if (graphMode !== '3d') return

    visibleGraphData.nodes.forEach((node) => {
      const object3D = (node as any).__threeObj as THREE.Object3D | undefined
      if (!object3D) return

      const highlighted = isNodeHighlighted(node.id)
      const dimmed = hasHoverSelection && !highlighted

      object3D.scale.setScalar(
        dimmed ? 0.85
        : highlighted ? 1.2
        : 1
      )

      object3D.traverse((child) => {
        const material = (child as any).material
        if (!material) return

        const materials = Array.isArray(material) ? material : [material]
        materials.forEach((mat: any) => {
          if (typeof mat.opacity === 'number') {
            mat.transparent = dimmed
            mat.opacity = dimmed ? 0.2 : 1
          }
          if (mat.emissive && typeof mat.emissive.setHex === 'function') {
            mat.emissive.setHex(
              dimmed ? 0x000000
              : highlighted && hasHoverSelection ? 0x1a1a1a
              : 0x000000
            )
          }
        })
      })
    })

    fgRef.current?.refresh?.()
  }, [graphMode, hasHoverSelection, isNodeHighlighted, visibleGraphData.nodes])

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

  const hideNode = useCallback((nodeId: string) => {
    setHiddenNodeIds((current) => (current.includes(nodeId) ? current : [...current, nodeId]))
  }, [])

  const handleClick = useCallback(
    (node: GraphNode, event?: MouseEvent) => {
      focusNode(node)

      if (event?.altKey) {
        hideNode(node.id)
        return
      }

      if (node.kind === 'record' && node.recordId) {
        $sheetProperty.set(undefined)
        $sheetRecordId.set(node.recordId)
        return
      }

      if (node.kind === 'property') {
        $sheetRecordId.set(undefined)
        $sheetProperty.set({
          key: node.id,
          name: node.propertyName ?? node.label,
          type: node.propertyType ?? 'string',
          labels: node.propertyLabels ?? [],
          vectorIndexed: !!node.vectorIndexed,
          connectedRecordIds: node.connectedRecordIds ?? []
        })
      }
    },
    [focusNode, hideNode]
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

  const resetHiddenNodes = useCallback(() => {
    setHiddenNodeIds([])
  }, [])

  const fitGraph = useCallback(() => {
    fgRef.current?.zoomToFit?.(400, 30)
  }, [])

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

  const createNodeObject3D = useCallback((node: GraphNode) => {
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

    // Keep record labels always visible in 3D with a dimmed color to reduce visual noise.
    const label = new SpriteText(node.label)
    label.color = '#9aa7b8'
    label.textHeight = 2.2
    label.position.set(0, -7.5, 0)
    group.add(label)

    return group
  }, [])

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

      // hide badge
      ctx.beginPath()
      ctx.fillStyle = dimmed ? withOpacity('#f43f5e', 0.2) : '#f43f5e'
      ctx.arc(x + 4.5, y - 4.5, 1.8, 0, Math.PI * 2)
      ctx.fill()
      ctx.strokeStyle = '#fff'
      ctx.lineWidth = 0.9
      ctx.beginPath()
      ctx.moveTo(x + 3.4, y - 3.4)
      ctx.lineTo(x + 5.6, y - 5.6)
      ctx.stroke()

      // vector badge
      if (node.kind === 'property' && node.vectorIndexed) {
        ctx.beginPath()
        ctx.fillStyle = dimmed ? withOpacity('#10b981', 0.2) : '#10b981'
        ctx.arc(x - 4.5, y - 4.5, 1.6, 0, Math.PI * 2)
        ctx.fill()
      }

      if (node.kind === 'record') {
        const fontSize = Math.max(8 / globalScale, 2.5)
        ctx.font = `${fontSize}px Sans-Serif`
        ctx.fillStyle = dimmed ? withOpacity('#9aa7b8', 0.2) : '#9aa7b8'
        ctx.textAlign = 'center'
        ctx.fillText(node.label, x, y + 9)
      }
      ctx.restore()
    },
    [hasHoverSelection, isNodeHighlighted]
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

  const renderCommonGraphControls = (
    <>
      <div className="bg-fill/90 border-content/30 absolute left-4 top-4 z-20 rounded-xl border p-3 shadow-xl backdrop-blur-sm">
        <div className="mb-2 text-xs font-semibold uppercase tracking-wide">Graph Layers</div>
        <div className="flex flex-col gap-2 text-sm">
          <label className="inline-flex items-center gap-2">
            <input checked={showProperties} onChange={() => setShowProperties((v) => !v)} type="checkbox" />
            Show Properties
          </label>
          <label className="inline-flex items-center gap-2">
            <input
              checked={showPropertyLinks}
              onChange={() => setShowPropertyLinks((v) => !v)}
              type="checkbox"
            />
            Show Property Links
          </label>
          <label className="inline-flex items-center gap-2">
            <input checked={showRecordLinks} onChange={() => setShowRecordLinks((v) => !v)} type="checkbox" />
            Show Record Links
          </label>
        </div>
      </div>

      <div className="border-content/30 bg-content text-fill animate-in fixed bottom-24 left-1/2 z-20 flex -translate-x-1/2 items-center overflow-clip rounded-2xl shadow-2xl ring">
        <Tooltip
          trigger={
            <Button
              className="rounded-none"
              onClick={() => setGraphMode((current) => (current === '3d' ? '2d' : '3d'))}
              size="small"
              variant="inverse"
            >
              {graphMode === '3d' ?
                <Square />
              : <Orbit />}
              {graphMode === '3d' ? 'Switch 2D' : 'Switch 3D'}
            </Button>
          }
        >
          <span>Toggle between 2D and 3D graph view</span>
        </Tooltip>

        <Button className="rounded-none" onClick={fitGraph} size="small" variant="inverse">
          <ScanSearch />
          Fit Graph
        </Button>

        <Button className="rounded-none" onClick={resetHiddenNodes} size="small" variant="inverse">
          <EyeOff />
          Reset Hidden
        </Button>

        <Button className="rounded-none" onClick={reheatSimulation} size="small" variant="inverse">
          <RotateCcw />
          Relayout
        </Button>
      </div>

      {selectedProperty && (
        <div className="bg-fill/90 border-content/30 absolute right-4 top-4 z-20 max-w-xs rounded-xl border p-3 shadow-xl backdrop-blur-sm">
          <div className="text-xs font-semibold uppercase tracking-wide">Selected Property</div>
          <div className="mt-1 text-sm font-medium">{selectedProperty.name}</div>
          <div className="text-content-secondary text-xs">Alt + click node to hide it</div>
        </div>
      )}
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

      {graphMode === '3d' ?
        <ForceGraph3D
          ref={fgRef}
          backgroundColor={'#222'}
          showNavInfo={false}
          graphData={visibleGraphData}
          linkWidth={(link: GraphLink) => {
            const highlighted = isLinkHighlighted(link.id)
            const base = link.kind === 'record-relation' ? 1.05 : 0.55
            return highlighted ? base * 1.2 : base
          }}
          linkOpacity={hasHoverSelection ? 0.14 : 0.3}
          linkColor={(link: GraphLink) => {
            const highlighted = isLinkHighlighted(link.id)
            const base = link.kind === 'record-relation' ? 'rgba(210,220,230,0.52)' : 'rgba(210,220,230,0.2)'
            return highlighted ? base : 'rgba(140,150,160,0.08)'
          }}
          nodeOpacity={1}
          nodeRelSize={6}
          nodeId={'id'}
          linkSource={'source'}
          linkTarget={'target'}
          nodeResolution={24}
          height={canvasSize.height}
          width={canvasSize.width}
          nodeColor={(node: GraphNode) => node.color}
          linkDirectionalArrowRelPos={1}
          linkDirectionalArrowLength={(link: GraphLink) => (link.kind === 'record-relation' ? 2.2 : 0)}
          linkDirectionalArrowResolution={16}
          nodeThreeObject={createNodeObject3D as any}
          onLinkClick={handleLinkClick}
          onNodeHover={(node: GraphNode | null) => {
            setHoveredLinkId(undefined)
            setHoveredNodeId(node?.id)
          }}
          onLinkHover={(link: GraphLink | null) => {
            setHoveredNodeId(undefined)
            setHoveredLinkId(link?.id)
          }}
          onNodeDragEnd={(node: any) => {
            node.fx = node.x
            node.fy = node.y
            node.fz = node.z

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
          onNodeRightClick={(node: GraphNode) => hideNode(node.id)}
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
          nodeCanvasObject={renderNode2D as any}
          nodePointerAreaPaint={renderNodePointerArea2D as any}
          cooldownTicks={120}
          onNodeHover={(node: GraphNode | null) => {
            setHoveredLinkId(undefined)
            setHoveredNodeId(node?.id)
          }}
          onLinkHover={(link: GraphLink | null) => {
            setHoveredNodeId(undefined)
            setHoveredLinkId(link?.id)
          }}
          onNodeDragEnd={(node: any) => {
            // 2D mode persists manually dragged positions.
            node.fx = node.x
            node.fy = node.y
            node.fz = undefined

            if (node?.id && Number.isFinite(node.x) && Number.isFinite(node.y)) {
              pinned2DPositionsRef.current[String(node.id)] = {
                x: node.x,
                y: node.y
              }
            }
          }}
          nodeLabel={getNodeHoverLabel as any}
          onNodeClick={handleClick as any}
          onNodeRightClick={(node: GraphNode) => hideNode(node.id)}
        />
      }
    </div>
  )
}
