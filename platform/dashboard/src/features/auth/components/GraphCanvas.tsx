import { useEffect, useRef } from 'react'

import { cn } from '~/lib/utils'

type Node = {
  x: number
  y: number
  vx: number
  vy: number
  r: number
  /** Independent wander phase/frequency so motion feels organic, not linear. */
  phase: number
  freq: number
}

const CONNECT_DISTANCE = 120
const MAX_SPEED = 0.32
const MIN_SPEED = 0.05
// One node per this many px² of canvas area, capped, so density stays even
// across viewport sizes.
const AREA_PER_NODE = 6500
const MAX_NODES = 180

function createNodes(width: number, height: number): Node[] {
  const count = Math.min(MAX_NODES, Math.max(24, Math.floor((width * height) / AREA_PER_NODE)))
  const nodes: Node[] = []
  for (let i = 0; i < count; i++) {
    const angle = Math.random() * Math.PI * 2
    const speed = MIN_SPEED + Math.random() * (MAX_SPEED - MIN_SPEED)
    nodes.push({
      x: Math.random() * width,
      y: Math.random() * height,
      vx: Math.cos(angle) * speed,
      vy: Math.sin(angle) * speed,
      r: 1 + Math.random() * 1.4,
      phase: Math.random() * Math.PI * 2,
      freq: 0.0004 + Math.random() * 0.0009
    })
  }
  return nodes
}

/**
 * Animated, grayscale "graph network" background — nodes drift around like a
 * lava lamp and link to nearby neighbours with fading edges. Purely decorative
 * (aria-hidden). Respects prefers-reduced-motion (renders a single static
 * frame) and pauses while the tab is hidden.
 */
export function GraphCanvas({ className }: { className?: string }) {
  const canvasRef = useRef<HTMLCanvasElement>(null)

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')
    if (!ctx) return

    const parent = canvas.parentElement ?? canvas
    const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches

    let nodes: Node[] = []
    let width = 0
    let height = 0
    let frame = 0
    let startTime = performance.now()

    const resize = () => {
      const dpr = Math.min(window.devicePixelRatio || 1, 2)
      const rect = parent.getBoundingClientRect()
      width = Math.max(1, rect.width)
      height = Math.max(1, rect.height)
      canvas.width = Math.floor(width * dpr)
      canvas.height = Math.floor(height * dpr)
      canvas.style.width = `${width}px`
      canvas.style.height = `${height}px`
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
      // Rebuild only when the node count would meaningfully change, otherwise
      // keep positions so a resize doesn't visibly reset the field.
      if (!nodes.length) nodes = createNodes(width, height)
    }

    const draw = (now: number) => {
      const elapsed = now - startTime
      ctx.clearRect(0, 0, width, height)

      // --- advance positions ---
      if (!reduceMotion) {
        for (const n of nodes) {
          // Gentle, ever-changing wander layered on the base velocity.
          const wander = Math.sin(elapsed * n.freq + n.phase) * 0.015
          n.vx += wander
          n.vy += Math.cos(elapsed * n.freq + n.phase) * 0.015

          const speed = Math.hypot(n.vx, n.vy)
          if (speed > MAX_SPEED) {
            n.vx = (n.vx / speed) * MAX_SPEED
            n.vy = (n.vy / speed) * MAX_SPEED
          }

          n.x += n.vx
          n.y += n.vy

          // Soft-bounce off the edges so the field stays in view.
          if (n.x < 0) {
            n.x = 0
            n.vx = Math.abs(n.vx)
          } else if (n.x > width) {
            n.x = width
            n.vx = -Math.abs(n.vx)
          }
          if (n.y < 0) {
            n.y = 0
            n.vy = Math.abs(n.vy)
          } else if (n.y > height) {
            n.y = height
            n.vy = -Math.abs(n.vy)
          }
        }
      }

      // --- edges ---
      for (let i = 0; i < nodes.length; i++) {
        for (let j = i + 1; j < nodes.length; j++) {
          const a = nodes[i]
          const b = nodes[j]
          const dx = a.x - b.x
          const dy = a.y - b.y
          const dist = Math.hypot(dx, dy)
          if (dist < CONNECT_DISTANCE) {
            const t = 1 - dist / CONNECT_DISTANCE
            ctx.strokeStyle = `rgba(190,190,190,${t * 0.2})`
            ctx.lineWidth = 1
            ctx.beginPath()
            ctx.moveTo(a.x, a.y)
            ctx.lineTo(b.x, b.y)
            ctx.stroke()
          }
        }
      }

      // --- nodes (flat grayscale dots, no glow) ---
      ctx.fillStyle = 'rgba(225,225,225,0.8)'
      for (const n of nodes) {
        ctx.beginPath()
        ctx.arc(n.x, n.y, n.r, 0, Math.PI * 2)
        ctx.fill()
      }

      if (!reduceMotion) frame = requestAnimationFrame(draw)
    }

    const start = () => {
      cancelAnimationFrame(frame)
      startTime = performance.now()
      frame = requestAnimationFrame(draw)
    }

    const onVisibility = () => {
      if (document.hidden) {
        cancelAnimationFrame(frame)
      } else if (!reduceMotion) {
        start()
      }
    }

    resize()
    const ro = new ResizeObserver(() => {
      resize()
    })
    ro.observe(parent)
    document.addEventListener('visibilitychange', onVisibility)

    if (reduceMotion) {
      draw(performance.now())
    } else {
      start()
    }

    return () => {
      cancelAnimationFrame(frame)
      ro.disconnect()
      document.removeEventListener('visibilitychange', onVisibility)
    }
  }, [])

  return <canvas ref={canvasRef} aria-hidden className={cn('block h-full w-full', className)} />
}
