import React, { useRef, useCallback, useState, useEffect, useLayoutEffect, Ref, RefObject } from 'react'

import { useGraphData } from '../useGraphData'
import { NodeObject } from 'react-force-graph-3d'
import { $sheetRecordId } from '../stores/id'
import { IconButton } from '~/elements/IconButton'
import { Expand } from 'lucide-react'
import { Switch } from '~/elements/Switch.tsx'

// Shared graph node/link types (mirrors 2DGraphView logic)
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

function stringToColor(str?: string) {
  if (!str) return '#64748B'
  let hash = 0
  for (let i = 0; i < str.length; i++) hash = str.charCodeAt(i) + ((hash << 5) - hash)
  const h = hash % 360
  return `hsl(${h},65%,45%)`
}

const color = (n: GraphNode, theme: 'light' | 'dark' = 'dark') =>
  n.type === 'property' ?
    theme === 'dark' ?
      '#6366F1'
    : '#4F46E5'
  : n.label ? stringToColor(n.label)
  : theme === 'dark' ? '#10B981'
  : '#059669'

const linkColor = (l: GraphLink, theme: 'light' | 'dark' = 'dark') =>
  l.type === 'value' ?
    theme === 'dark' ?
      'rgba(99,102,241,0.35)'
    : 'rgba(79,70,229,0.4)'
  : theme === 'dark' ? 'rgba(148,163,184,0.45)'
  : 'rgba(100,116,139,0.5)'

// Simple ResizeObserver hook (copied from 2D view)
function useResizeObserver<E extends HTMLDivElement | null = null>(
  ref: React.RefObject<E>,
  cb: (rect: DOMRectReadOnly) => void
) {
  useLayoutEffect(() => {
    if (!ref.current) return
    const el = ref.current
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) cb(entry.contentRect)
    })
    ro.observe(el)
    return () => ro.disconnect()
  }, [ref, cb])
}

export const GraphView = () => {
  const {
    graphData,
    showProperties,
    setShowProperties,
    loading,
    totalRecordsLoaded,
    totalRelationsLoaded,
    hasMoreRecords,
    hasMoreRelationships,
    loadMoreRecords,
    loadMoreRelationships,
    maxPossibleRecords,
    recordsTotal,
    relationsTotal,
    recordsLoadingProgress,
    relationsLoadingProgress
  } = useGraphData()

  const [mode, setMode] = useState<'2d' | '3d'>('2d') // default 2D

  // dynamic imports
  const [FG2D, setFG2D] = useState<any>(null)
  const [FG3D, setFG3D] = useState<any>(null)
  useEffect(() => {
    let cancelled = false
    import('react-force-graph-2d').then((m) => !cancelled && setFG2D(() => m.default))
    // defer 3D until needed
    if (mode === '3d' && !FG3D) {
      import('react-force-graph-3d').then((m) => !cancelled && setFG3D(() => m.default))
    }
    return () => {
      cancelled = true
    }
  }, [mode, FG3D])

  const containerRef = useRef<HTMLDivElement | null>(null)
  const [dims, setDims] = useState({ w: 800, h: 600 })
  useResizeObserver(containerRef, (rect) => {
    const w = Math.max(100, rect.width)
    const h = Math.max(100, rect.height)
    setDims((d) => (d.w === w && d.h === h ? d : { w, h }))
  })

  const fg2dRef = useRef<any>(null)
  const fg3dRef = useRef<any>(null)

  const zoomToFit = useCallback(() => {
    try {
      if (mode === '2d' && fg2dRef.current?.zoomToFit) fg2dRef.current.zoomToFit(600, 60)
      if (mode === '3d' && fg3dRef.current?.zoomToFit) fg3dRef.current.zoomToFit(600, 60)
    } catch {}
  }, [mode])

  useEffect(() => {
    // Auto-fit zoom when graph data changes significantly
    if (graphData.nodes.length > 0) {
      const timeoutId = setTimeout(zoomToFit, 1000)
      return () => clearTimeout(timeoutId)
    }
  }, [graphData.nodes.length, zoomToFit])

  // Hover highlight (2D only for now)
  const [hoverNode, setHoverNode] = useState<GraphNode | null>(null)
  const [highlightNodes, setHighlightNodes] = useState<Set<GraphNode>>(new Set())
  const [highlightLinks, setHighlightLinks] = useState<Set<GraphLink>>(new Set())

  const handleNodeHover = useCallback((node: GraphNode | null) => {
    setHoverNode(node)
    const newHNodes = new Set<GraphNode>()
    const newHLinks = new Set<GraphLink>()
    if (node) {
      newHNodes.add(node)
      node.neighbors?.forEach((n) => newHNodes.add(n))
      node.links?.forEach((l) => newHLinks.add(l))
    }
    setHighlightNodes(newHNodes)
    setHighlightLinks(newHLinks)
  }, [])

  const nodeCanvasObject = useCallback(
    (node: GraphNode, ctx: CanvasRenderingContext2D, globalScale: number) => {
      const radius = Math.max(4, Math.sqrt(node.val || 4) * 4)
      ctx.beginPath()
      ctx.fillStyle = color(node)
      ctx.arc(node.x!, node.y!, radius, 0, 2 * Math.PI, false)
      ctx.fill()
      if (hoverNode === node) {
        const label = node.name || node.label || node.id
        const fontSize = 12 / globalScale
        ctx.font = `${fontSize}px sans-serif`
        ctx.textAlign = 'center'
        ctx.textBaseline = 'top'
        ctx.fillStyle = '#f1f5f9'
        ctx.fillText(label, node.x!, node.y! + radius + 2)
      }
      if (highlightNodes.has(node)) {
        const radius2 = radius * 1.1
        ctx.beginPath()
        ctx.arc(node.x!, node.y!, radius2, 0, 2 * Math.PI, false)
        ctx.strokeStyle = node === hoverNode ? '#3f81ff' : 'orange'
        ctx.lineWidth = 2
        ctx.stroke()
      }
    },
    [hoverNode, highlightNodes]
  )

  return (
    <div
      ref={containerRef}
      className="bg-fill absolute bottom-0 w-full"
      style={{ height: `calc(100vh - 182px)` }}
    >
      {!loading && mode === '2d' && FG2D && (
        <FG2D
          ref={fg2dRef}
          graphData={graphData}
          width={dims.w}
          height={dims.h}
          forceEngine="ngraph"
          nodeId="id"
          nodeLabel="name"
          linkColor={linkColor}
          nodeCanvasObject={nodeCanvasObject}
          onNodeHover={handleNodeHover}
          onNodeDragEnd={(node: NodeObject) => {
            node.fx = node.x
            node.fy = node.y
          }}
          cooldownTime={8000}
          enableNodeDrag
          onNodeClick={(node: GraphNode) => {
            if (node.type !== 'property') {
              $sheetRecordId.set(node.id)
            }
          }}
          onLinkHover={(l: GraphLink | null) => {
            if (!l) {
              setHighlightLinks(new Set())
              return
            }
            const newHLinks = new Set<GraphLink>([l])
            const newHNodes = new Set<GraphNode>()
            if (typeof l.source !== 'string') newHNodes.add(l.source as GraphNode)
            if (typeof l.target !== 'string') newHNodes.add(l.target as GraphNode)
            setHighlightLinks(newHLinks)
            setHighlightNodes(newHNodes)
          }}
        />
      )}
      {!loading && mode === '3d' && FG3D && (
        <FG3D
          ref={fg3dRef}
          graphData={graphData}
          width={dims.w}
          height={dims.h}
          nodeId="id"
          linkTarget="target"
          linkSource="source"
          linkColor={linkColor}
          onNodeDragEnd={(node: NodeObject) => {
            node.fx = node.x
            node.fy = node.y
            node.fz = node.z
          }}
          nodeRelSize={6}
          linkOpacity={0.25}
          onNodeClick={(n: any) => {
            // center camera
            const distance = 260
            const distRatio = 1 + distance / Math.hypot(n.x, n.y, n.z)
            fg3dRef.current?.cameraPosition(
              { x: n.x * distRatio, y: n.y * distRatio, z: n.z * distRatio },
              n,
              1000
            )
          }}
        />
      )}

      {loading && (
        <div className="absolute inset-0 flex items-center justify-center bg-black/20 backdrop-blur-sm">
          <div className="rounded-lg bg-black/70 p-6 text-center text-white backdrop-blur">
            <div className="mb-4">
              <div className="mx-auto h-8 w-8 animate-spin rounded-full border-2 border-white border-t-transparent" />
            </div>
            <div className="mb-2 text-lg font-semibold">Loading Graph Data</div>
            <div className="text-sm text-gray-300">
              Fetching {Math.min(recordsTotal || 0, maxPossibleRecords).toLocaleString()} records and
              relations...
            </div>
            <div className="mt-4 space-y-2">
              <div className="flex items-center justify-between text-xs">
                <span>Records: {totalRecordsLoaded.toLocaleString()}</span>
                <span>{Math.round(recordsLoadingProgress)}%</span>
              </div>
              <div className="h-1 w-48 overflow-hidden rounded-full bg-gray-700">
                <div
                  className="h-full bg-blue-400 transition-all duration-300"
                  style={{ width: `${recordsLoadingProgress}%` }}
                />
              </div>
              <div className="flex items-center justify-between text-xs">
                <span>Relations: {totalRelationsLoaded.toLocaleString()}</span>
                <span>{Math.round(relationsLoadingProgress)}%</span>
              </div>
              <div className="h-1 w-48 overflow-hidden rounded-full bg-gray-700">
                <div
                  className="h-full bg-green-400 transition-all duration-300"
                  style={{ width: `${relationsLoadingProgress}%` }}
                />
              </div>
            </div>
          </div>
        </div>
      )}

      {!loading && (
        <div className="pointer-events-auto absolute left-4 top-4 z-20 rounded-lg bg-black/50 px-3 py-2 text-xs text-white backdrop-blur">
          <div className="flex flex-col gap-1">
            <div className="flex items-center gap-2">
              <span>Records: {totalRecordsLoaded.toLocaleString()}</span>
              {recordsTotal && (
                <span className="text-gray-400">
                  / {Math.min(recordsTotal, maxPossibleRecords).toLocaleString()}
                </span>
              )}
              {totalRecordsLoaded >= maxPossibleRecords && <span className="text-amber-300">(max)</span>}
            </div>
            <div className="flex items-center gap-2">
              <span>Relations: {totalRelationsLoaded.toLocaleString()}</span>
              {relationsTotal && (
                <span className="text-gray-400">
                  / {Math.min(relationsTotal, maxPossibleRecords).toLocaleString()}
                </span>
              )}
            </div>
            {(hasMoreRecords || hasMoreRelationships) && (
              <button
                onClick={() => {
                  if (hasMoreRecords) loadMoreRecords()
                  if (hasMoreRelationships) loadMoreRelationships()
                }}
                className="text-left text-blue-300 hover:text-blue-200"
              >
                Load more
              </button>
            )}
          </div>
        </div>
      )}

      <div className="pointer-events-auto absolute bottom-4 left-1/2 z-20 flex -translate-x-1/2 gap-4 rounded-full bg-black/50 px-8 py-2 text-xs text-white backdrop-blur">
        {/* <IconButton
          aria-label="2d"
          variant={mode === '2d' ? 'primary' : 'secondary'}
          size="small"
          onClick={() => setMode('2d')}
        >
          2D
        </IconButton>
        <IconButton
          aria-label="3d"
          className="font-bold"
          variant={mode === '3d' ? 'primary' : 'secondary'}
          size="small"
          onClick={() => setMode('3d')}
        >
          3D
        </IconButton> */}
        <IconButton
          aria-label="3d"
          className="font-bold"
          variant="secondary"
          size="small"
          onClick={zoomToFit}
        >
          <Expand />
        </IconButton>
        <div className="bg-stroke w-px self-stretch" />
        <label className="flex cursor-pointer items-center gap-1">
          <p className="text-content2 typography-base font-medium">Show properties</p>
          <Switch checked={showProperties} onCheckedChange={(checked) => setShowProperties(checked)} />
        </label>
      </div>
    </div>
  )
}

export default GraphView
