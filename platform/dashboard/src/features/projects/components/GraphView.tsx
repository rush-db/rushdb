import React, { useRef, useCallback, FC, useState, useEffect } from 'react'

import { CSS2DRenderer } from 'three/addons/renderers/CSS2DRenderer.js'
import { ForceGraph3D } from 'react-force-graph'
import SpriteText from 'three-spritetext'
import { useStore } from '@nanostores/react'

import {
  $currentProjectLabels,
  $filteredRecordsRelations
} from '~/features/projects/stores/current-project.ts'
import { Renderer } from 'three'
import { $sheetRecordId } from '~/features/projects/stores/id.ts'
import { getLabelColor } from '~/features/labels'
import { DBRecord, type Relation } from '@rushdb/javascript-sdk'

export function getRelationCounts(tree: Output) {
  const counts: Record<string, number> = {}

  tree.nodes?.forEach((node) => {
    counts[node.__id] = 0
  })

  tree.links?.forEach((link) => {
    const source = link.sourceId
    const target = link.targetId

    counts[source] = Math.min((counts[source] || 0) + 1, 64)
    counts[target] = Math.min((counts[target] || 0) + 1, 64)
  })

  return counts
}

interface Output {
  nodes: DBRecord[]
  links: Relation[]
}

function createNodesAndLinks(data: Relation[]): Output {
  const nodesMap = new Map<string, DBRecord>()

  data.forEach(({ sourceId, sourceLabel, targetId, targetLabel }) => {
    if (!nodesMap.has(sourceId)) {
      nodesMap.set(sourceId, { __id: sourceId, __label: sourceLabel })
    }
    if (!nodesMap.has(targetId)) {
      nodesMap.set(targetId, { __id: targetId, __label: targetLabel })
    }
  })

  return {
    nodes: Array.from(nodesMap.values()),
    links: data
  }
}

const HEADER_HEIGHT = 182
const FOOTER_HEIGHT = 61

export const GraphView: FC = () => {
  const extraRenderers: Renderer[] = [new CSS2DRenderer() as unknown as Renderer]

  const { data: relations } = useStore($filteredRecordsRelations)

  const [data, setData] = useState<Output>({ nodes: [], links: [] })
  const [weights, setWeights] = useState<Record<string, number>>({})

  useEffect(() => {
    const result = createNodesAndLinks(relations ?? [])
    setData(result)

    const newWeights = getRelationCounts(result)
    setWeights(newWeights)
  }, [relations])

  const fgRef = useRef<any>()

  const handleClick = useCallback(
    (node: any) => {
      const distance = 260
      const distRatio = 1 + distance / Math.hypot(node.x, node.y, node.z)

      fgRef.current?.cameraPosition(
        { x: node.x * distRatio, y: node.y * distRatio, z: node.z * distRatio },
        node,
        1000
      )

      $sheetRecordId.set(node.__id)
    },
    [fgRef]
  )

  const handleLinkClick = useCallback(
    (link: any) => {
      const distance = 70

      const sx = link.source.x
      const sy = link.source.y
      const sz = link.source.z

      const tx = link.target.x
      const ty = link.target.y
      const tz = link.target.z

      const dx = sx - tx
      const dy = sy - ty
      const dz = sz - tz

      const rdx = dz
      const rdy = dy
      const rdz = -dx

      const dist = Math.hypot(rdx, rdy, rdz)

      const distRatio = 1 + distance / dist

      const mx = (sx + tx) / 2
      const my = (sy + ty) / 2
      const mz = (sz + tz) / 2

      const cx = mx + rdx * distRatio
      const cy = my + rdy * distRatio
      const cz = mz + rdz * distRatio

      fgRef.current?.cameraPosition({ x: cx, y: cy, z: cz }, { x: mx, y: my, z: mz }, 1000)
    },
    [fgRef]
  )
  const [canvasSize, setCanvasSize] = useState({
    width: window.innerWidth - 16,
    height: window.innerHeight - HEADER_HEIGHT - FOOTER_HEIGHT
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

  return (
    <ForceGraph3D
      ref={fgRef}
      backgroundColor={'#222'}
      extraRenderers={extraRenderers}
      showNavInfo={false}
      graphData={data}
      linkWidth={1.5}
      linkOpacity={0.2}
      nodeOpacity={1}
      nodeRelSize={6}
      nodeId={'__id'}
      linkSource={'sourceId'}
      linkTarget={'targetId'}
      nodeResolution={24}
      height={canvasSize.height}
      width={canvasSize.width}
      nodeVal={(node: DBRecord) => weights[node.__id as string]}
      linkDirectionalArrowRelPos={1}
      linkDirectionalArrowLength={4}
      linkDirectionalArrowResolution={16}
      nodeThreeObjectExtend={true}
      onLinkClick={handleLinkClick}
      nodeColor={(node) => {
        const { data: labels } = $currentProjectLabels.get()
        return getLabelColor(node.__label, Object.keys(labels ?? {}).indexOf(node.__label))
      }}
      onNodeDragEnd={(node) => {
        node.fx = node.x
        node.fy = node.y
        node.fz = node.z
      }}
      linkThreeObjectExtend={true}
      linkThreeObject={(link) => {
        const sprite = new SpriteText(`${link.sourceLabel} > ${link.targetLabel}`)
        sprite.color = 'lightgrey'
        sprite.textHeight = 1.5
        return sprite
      }}
      linkPositionUpdate={(sprite, { start, end }) => {
        const middlePos = Object.assign(
          // @ts-ignore
          ...['x', 'y', 'z'].map((c) => ({
            // @ts-ignore
            [c]: start[c] + (end[c] - start[c]) / 2 // calc middle point
          }))
        )

        // Position sprite
        Object.assign(sprite.position, middlePos)
      }}
      nodeLabel={(node) => `<div><b>${node.__label}</b>: <span>${node.__id}</span></div>`}
      onNodeClick={handleClick}
    />
  )
}
