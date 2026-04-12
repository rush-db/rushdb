import { useEffect, useRef } from 'react'

// ── Shape table ──────────────────────────────────────────────────────────────
// Each shape is { tl, tr, br, bl } where each value is 0 (square) or 0.5 (half)
// representing fraction of (cellSize/2) used as the arc radius.
interface CornerShape {
  tl: number
  tr: number
  br: number
  bl: number
}

const SHAPES: CornerShape[] = [
  { tl: 0, tr: 0, br: 0, bl: 0 }, // square
  { tl: 0.5, tr: 0, br: 0, bl: 0 }, // TL only
  { tl: 0, tr: 0.5, br: 0, bl: 0 }, // TR only
  { tl: 0, tr: 0, br: 0.5, bl: 0 }, // BR only
  { tl: 0, tr: 0, br: 0, bl: 0.5 }, // BL only
  { tl: 0.5, tr: 0.5, br: 0, bl: 0 }, // top stadium
  { tl: 0, tr: 0, br: 0.5, bl: 0.5 }, // bottom stadium
  { tl: 0.5, tr: 0, br: 0, bl: 0.5 }, // left stadium
  { tl: 0, tr: 0.5, br: 0.5, bl: 0 }, // right stadium
  { tl: 0.5, tr: 0, br: 0.5, bl: 0 }, // diagonal TL+BR
  { tl: 0, tr: 0.5, br: 0, bl: 0.5 }, // diagonal TR+BL
  { tl: 0.5, tr: 0.5, br: 0.5, bl: 0.5 }, // circle
  { tl: 0, tr: 0.5, br: 0.5, bl: 0.5 }, // all but TL
  { tl: 0.5, tr: 0, br: 0.5, bl: 0.5 }, // all but TR
  { tl: 0.5, tr: 0.5, br: 0, bl: 0.5 }, // all but BR
  { tl: 0.5, tr: 0.5, br: 0.5, bl: 0 } // all but BL
]

// R easter egg — 3×3 row-major (same mapping as original)
const R_EGG_SHAPES: CornerShape[] = [
  { tl: 0, tr: 0, br: 0, bl: 0.5 },
  { tl: 0, tr: 0, br: 0.5, bl: 0 },
  { tl: 0, tr: 0.5, br: 0, bl: 0.5 },
  { tl: 0.5, tr: 0.5, br: 0.5, bl: 0.5 },
  { tl: 0, tr: 0.5, br: 0, bl: 0 },
  { tl: 0.5, tr: 0, br: 0.5, bl: 0 },
  { tl: 0.5, tr: 0.5, br: 0.5, bl: 0.5 },
  { tl: 0, tr: 0, br: 0, bl: 0 },
  { tl: 0, tr: 0.5, br: 0, bl: 0.5 }
]

function pickShape(exclude?: CornerShape): CornerShape {
  const pool =
    exclude ?
      SHAPES.filter(
        (s) => !(s.tl === exclude.tl && s.tr === exclude.tr && s.br === exclude.br && s.bl === exclude.bl)
      )
    : SHAPES
  return pool[Math.floor(Math.random() * pool.length)]
}

// ── Cell data ────────────────────────────────────────────────────────────────
interface Cell {
  // geometry
  x: number
  y: number
  size: number
  // animated corner fractions (0–0.5)
  tl: number
  tr: number
  br: number
  bl: number
  // targets
  tlT: number
  trT: number
  brT: number
  blT: number
  // lerp speed per frame (varies cell-to-cell for organic feel)
  speed: number
  // border colour (0–255)
  bR: number
  bG: number
  bB: number
  bRT: number
  bGT: number
  bBT: number
  bSpeed: number
}

// ── Constants ────────────────────────────────────────────────────────────────
const GAP = 0 // no gap — adjacent cell borders abut (matches CSS border-box)
const TICK_MS = 1400
const FRAC_LO = 0.05
const FRAC_HI = 0.1
const DUR_MIN_FRAMES = 210 // 3500ms @ 60fps
const DUR_MAX_FRAMES = 420 // 7000ms @ 60fps
const HOVER_FRAMES = 24 // 400ms
const NEIGHBOR_FRAMES = 25 // 420ms
const RESTORE_FRAMES_BASE = 33 // 550ms
const R_HOLD_MS = 1800
const SPOT_RADIUS = 2

const DEFAULT_BR = 0x1e
const DEFAULT_BG = 0x1e
const DEFAULT_BB = 0x22

function readBorderRgb(): [number, number, number] {
  if (typeof window === 'undefined') return [DEFAULT_BR, DEFAULT_BG, DEFAULT_BB]
  const raw = getComputedStyle(document.documentElement).getPropertyValue('--lp-border').trim()
  if (raw && raw.startsWith('#') && raw.length === 7) return parseHex(raw)
  return [DEFAULT_BR, DEFAULT_BG, DEFAULT_BB]
}

function speedFromFrames(frames: number): number {
  // lerp speed so that after `frames` frames the value is ~99% of the way there
  // speed = 1 - Math.pow(0.01, 1/frames)
  return 1 - Math.pow(0.01, 1 / frames)
}

function parseHex(hex: string): [number, number, number] {
  const h = hex.replace('#', '')
  return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)]
}

// ── Rounded-rect path ────────────────────────────────────────────────────────
function drawCell(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  tl: number,
  tr: number,
  br: number,
  bl: number
) {
  ctx.beginPath()
  ctx.moveTo(x + tl, y)
  ctx.lineTo(x + w - tr, y)
  ctx.arcTo(x + w, y, x + w, y + tr, tr)
  ctx.lineTo(x + w, y + h - br)
  ctx.arcTo(x + w, y + h, x + w - br, y + h, br)
  ctx.lineTo(x + bl, y + h)
  ctx.arcTo(x, y + h, x, y + h - bl, bl)
  ctx.lineTo(x, y + tl)
  ctx.arcTo(x, y, x + tl, y, tl)
  ctx.closePath()
}

// ── Component ────────────────────────────────────────────────────────────────
interface HeroGridProps {
  spotlight?: boolean
}

export function HeroGridCanvas({ spotlight = true }: HeroGridProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const canvasRef = useRef<HTMLCanvasElement>(null)

  // All mutable state lives in refs — zero React re-renders during animation
  const cells = useRef<Cell[]>([])
  const cols = useRef(0)
  const rows = useRef(0)
  const cellSize = useRef(0)
  const rafId = useRef<number>(0)
  const tickRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const lastCellIdx = useRef(-1)
  // hover tracking
  const hoverActive = useRef<Set<number>>(new Set())
  const preHoverShape = useRef<CornerShape[]>([])
  // lava-lamp tracking (just a set of indices currently morphing via lava)
  const lavaActive = useRef<Set<number>>(new Set())
  // R egg
  const rEggCells = useRef<Set<number>>(new Set())
  const rEggTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const staggerTimers = useRef<ReturnType<typeof setTimeout>[]>([])

  const accentRgb = useRef<[number, number, number]>(parseHex('#C8540A'))
  const borderRgb = useRef<[number, number, number]>([DEFAULT_BR, DEFAULT_BG, DEFAULT_BB])

  // ── Sync border colour from CSS variable (theme-aware) ──────────────────
  useEffect(() => {
    const sync = () => {
      const prev = borderRgb.current
      const [r, g, b] = readBorderRgb()
      borderRgb.current = [r, g, b]
      // sync accent from --lp-accent
      const rawAccent = getComputedStyle(document.documentElement).getPropertyValue('--lp-accent').trim()
      if (rawAccent && rawAccent.startsWith('#') && rawAccent.length === 7) {
        accentRgb.current = parseHex(rawAccent)
      }
      // push new default target to all cells not currently accented
      cells.current.forEach((cell) => {
        if (
          Math.abs(cell.bRT - prev[0]) < 4 &&
          Math.abs(cell.bGT - prev[1]) < 4 &&
          Math.abs(cell.bBT - prev[2]) < 4
        ) {
          cell.bRT = r
          cell.bGT = g
          cell.bBT = b
        }
      })
    }
    sync()
    const mo = new MutationObserver(sync)
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
    return () => mo.disconnect()
  }, [])

  // ── Build / rebuild cells array ─────────────────────────────────────────
  function buildCells(c: number, r: number, sz: number) {
    const [br, bg, bb] = borderRgb.current
    const newCells: Cell[] = []
    for (let row = 0; row < r; row++) {
      for (let col = 0; col < c; col++) {
        const shape = pickShape()
        newCells.push({
          x: col * (sz + GAP),
          y: row * (sz + GAP),
          size: sz,
          tl: shape.tl * sz,
          tr: shape.tr * sz,
          br: shape.br * sz,
          bl: shape.bl * sz,
          tlT: shape.tl * sz,
          trT: shape.tr * sz,
          brT: shape.br * sz,
          blT: shape.bl * sz,
          speed: speedFromFrames(DUR_MIN_FRAMES + Math.random() * (DUR_MAX_FRAMES - DUR_MIN_FRAMES)),
          bR: br,
          bG: bg,
          bB: bb,
          bRT: br,
          bGT: bg,
          bBT: bb,
          bSpeed: speedFromFrames(7) // ~120ms border colour
        })
      }
    }
    return newCells
  }

  // ── Resize + DPR handling ────────────────────────────────────────────────
  useEffect(() => {
    const container = containerRef.current
    const canvas = canvasRef.current
    if (!container || !canvas) return

    const resize = () => {
      const dpr = window.devicePixelRatio || 1
      const w = container.offsetWidth
      const h = container.offsetHeight
      if (w === 0 || h === 0) return

      const newCols =
        w < 640 ? 8
        : w < 1920 ? 16
        : 24
      const sz = Math.floor(w / newCols)
      const newRows = Math.ceil(h / sz) + 2

      canvas.width = w * dpr
      canvas.height = newRows * (sz + GAP) * dpr
      canvas.style.width = `${w}px`
      canvas.style.height = `${newRows * (sz + GAP)}px`

      const ctx = canvas.getContext('2d')
      if (ctx) ctx.scale(dpr, dpr)

      cols.current = newCols
      rows.current = newRows
      cellSize.current = sz
      cells.current = buildCells(newCols, newRows, sz)
      preHoverShape.current = cells.current.map((c) => ({
        tl: c.tl / sz,
        tr: c.tr / sz,
        br: c.br / sz,
        bl: c.bl / sz
      }))
      hoverActive.current.clear()
      lavaActive.current.clear()
      lastCellIdx.current = -1
    }

    resize()
    const ro = new ResizeObserver(resize)
    ro.observe(container)
    return () => ro.disconnect()
  }, [])

  // ── rAF draw loop ────────────────────────────────────────────────────────
  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')
    if (!ctx) return

    const EPS = 0.15 // px — stop lerping below this

    const frame = () => {
      const dpr = window.devicePixelRatio || 1
      ctx.clearRect(0, 0, canvas.width / dpr, canvas.height / dpr)

      // Two-pass render: default-colour cells first (batch), then accent cells
      // This lets us do only 2 strokeStyle changes per frame in most cases.
      const accentIndices: number[] = []

      cells.current.forEach((cell, i) => {
        // lerp corners
        const d = cell.size / 2
        cell.tl = Math.abs(cell.tlT - cell.tl) < EPS ? cell.tlT : cell.tl + (cell.tlT - cell.tl) * cell.speed
        cell.tr = Math.abs(cell.trT - cell.tr) < EPS ? cell.trT : cell.tr + (cell.trT - cell.tr) * cell.speed
        cell.br = Math.abs(cell.brT - cell.br) < EPS ? cell.brT : cell.br + (cell.brT - cell.br) * cell.speed
        cell.bl = Math.abs(cell.blT - cell.bl) < EPS ? cell.blT : cell.bl + (cell.blT - cell.bl) * cell.speed

        // lerp border colour
        cell.bR = Math.abs(cell.bRT - cell.bR) < 0.5 ? cell.bRT : cell.bR + (cell.bRT - cell.bR) * cell.bSpeed
        cell.bG = Math.abs(cell.bGT - cell.bG) < 0.5 ? cell.bGT : cell.bG + (cell.bGT - cell.bG) * cell.bSpeed
        cell.bB = Math.abs(cell.bBT - cell.bB) < 0.5 ? cell.bBT : cell.bB + (cell.bBT - cell.bB) * cell.bSpeed

        const isDefault =
          Math.abs(cell.bR - borderRgb.current[0]) < 2 &&
          Math.abs(cell.bG - borderRgb.current[1]) < 2 &&
          Math.abs(cell.bB - borderRgb.current[2]) < 2

        if (!isDefault) {
          accentIndices.push(i)
        }
      })

      // Pass 1 — default border colour (single strokeStyle)
      ctx.strokeStyle = `rgb(${borderRgb.current[0]},${borderRgb.current[1]},${borderRgb.current[2]})`
      ctx.lineWidth = 1
      cells.current.forEach((cell, i) => {
        if (accentIndices.includes(i)) return
        drawCell(ctx, cell.x + 0.5, cell.y + 0.5, cell.size, cell.size, cell.tl, cell.tr, cell.br, cell.bl)
        ctx.stroke()
      })

      // Pass 2 — accent-coloured cells (one strokeStyle per unique colour, rare)
      for (const i of accentIndices) {
        const cell = cells.current[i]
        ctx.strokeStyle = `rgb(${cell.bR | 0},${cell.bG | 0},${cell.bB | 0})`
        drawCell(ctx, cell.x + 0.5, cell.y + 0.5, cell.size, cell.size, cell.tl, cell.tr, cell.br, cell.bl)
        ctx.stroke()
      }

      rafId.current = requestAnimationFrame(frame)
    }

    rafId.current = requestAnimationFrame(frame)
    return () => cancelAnimationFrame(rafId.current)
  }, [])

  // ── Lava-lamp tick ───────────────────────────────────────────────────────
  useEffect(() => {
    const tick = () => {
      const total = cells.current.length
      if (total === 0) return
      const frac = FRAC_LO + Math.random() * (FRAC_HI - FRAC_LO)
      const count = Math.floor(total * frac)

      const candidates: number[] = []
      for (let i = 0; i < total; i++) {
        if (!lavaActive.current.has(i) && !hoverActive.current.has(i)) candidates.push(i)
      }
      // shuffle
      for (let i = candidates.length - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1))
        ;[candidates[i], candidates[j]] = [candidates[j], candidates[i]]
      }

      candidates.slice(0, count).forEach((i) => {
        const cell = cells.current[i]
        if (!cell) return
        const sz = cell.size
        const shape = pickShape({
          tl: cell.tlT / sz,
          tr: cell.trT / sz,
          br: cell.brT / sz,
          bl: cell.blT / sz
        })
        const frames = DUR_MIN_FRAMES + Math.random() * (DUR_MAX_FRAMES - DUR_MIN_FRAMES)
        cell.tlT = shape.tl * sz
        cell.trT = shape.tr * sz
        cell.brT = shape.br * sz
        cell.blT = shape.bl * sz
        cell.speed = speedFromFrames(frames)
        lavaActive.current.add(i)
        // remove from lava set after estimated duration
        setTimeout(() => lavaActive.current.delete(i), frames * (1000 / 60))
      })
    }

    const firstTick = setTimeout(tick, 400)
    tickRef.current = setInterval(tick, TICK_MS)
    return () => {
      clearTimeout(firstTick)
      if (tickRef.current) clearInterval(tickRef.current)
    }
  }, [])

  // ── Mouse + click interaction ────────────────────────────────────────────
  useEffect(() => {
    const container = containerRef.current
    if (!container) return

    function setTarget(i: number, shape: CornerShape, speed: number) {
      const cell = cells.current[i]
      if (!cell) return
      const sz = cell.size
      cell.tlT = shape.tl * sz
      cell.trT = shape.tr * sz
      cell.brT = shape.br * sz
      cell.blT = shape.bl * sz
      cell.speed = speed
    }

    function setBorderTarget(i: number, r: number, g: number, b: number, speed: number) {
      const cell = cells.current[i]
      if (!cell) return
      cell.bRT = r
      cell.bGT = g
      cell.bBT = b
      cell.bSpeed = speed
    }

    function morphHover(i: number, frames: number) {
      const cell = cells.current[i]
      if (!cell) return
      const sz = cell.size
      if (!hoverActive.current.has(i)) {
        preHoverShape.current[i] = {
          tl: cell.tlT / sz,
          tr: cell.trT / sz,
          br: cell.brT / sz,
          bl: cell.blT / sz
        }
        hoverActive.current.add(i)
      }
      const shape = pickShape({
        tl: cell.tlT / sz,
        tr: cell.trT / sz,
        br: cell.brT / sz,
        bl: cell.blT / sz
      })
      setTarget(i, shape, speedFromFrames(frames))
    }

    const cancelREgg = () => {
      if (rEggTimer.current) clearTimeout(rEggTimer.current)
      staggerTimers.current.forEach(clearTimeout)
      staggerTimers.current = []
      rEggCells.current.forEach((i) => {
        const cell = cells.current[i]
        if (!cell) return
        const restore = preHoverShape.current[i]
        if (restore) {
          setTarget(i, restore, speedFromFrames(RESTORE_FRAMES_BASE))
        }
        setBorderTarget(
          i,
          borderRgb.current[0],
          borderRgb.current[1],
          borderRgb.current[2],
          speedFromFrames(30)
        )
        hoverActive.current.delete(i)
      })
      rEggCells.current.clear()
    }

    const restoreAll = () => {
      cancelREgg()
      hoverActive.current.forEach((i) => {
        const restore = preHoverShape.current[i]
        if (restore) {
          const frames = RESTORE_FRAMES_BASE + Math.random() * 18
          setTarget(i, restore, speedFromFrames(frames))
        }
      })
      hoverActive.current.clear()
      lastCellIdx.current = -1
      // restore all spotlight borders
      for (let i = 0; i < cells.current.length; i++) {
        setBorderTarget(
          i,
          borderRgb.current[0],
          borderRgb.current[1],
          borderRgb.current[2],
          speedFromFrames(24)
        )
      }
    }

    const handleMove = (e: MouseEvent) => {
      const rect = container.getBoundingClientRect()
      const sz = cellSize.current
      if (sz === 0) return

      const inside =
        e.clientX >= rect.left && e.clientX <= rect.right && e.clientY >= rect.top && e.clientY <= rect.bottom

      if (!inside) {
        if (lastCellIdx.current !== -1) restoreAll()
        return
      }

      const col = Math.floor((e.clientX - rect.left) / (sz + GAP))
      const row = Math.floor((e.clientY - rect.top) / (sz + GAP))
      const idx = row * cols.current + col

      if (idx < 0 || idx >= cells.current.length) return

      // Cancel R egg only on exit from its 3×3 block
      if (rEggCells.current.size > 0 && !rEggCells.current.has(idx)) {
        cancelREgg()
      }

      if (idx === lastCellIdx.current) return
      lastCellIdx.current = idx

      morphHover(idx, HOVER_FRAMES)

      const neighbors = [idx - cols.current, idx + cols.current, idx - 1, idx + 1].filter(
        (n) => n >= 0 && n < cells.current.length
      )
      neighbors.forEach((n, ni) => {
        setTimeout(() => morphHover(n, NEIGHBOR_FRAMES), 60 + ni * 20)
      })

      if (!spotlight) return

      const centerRow = Math.floor(idx / cols.current)
      const centerCol = idx % cols.current

      for (let i = 0; i < cells.current.length; i++) {
        const r = Math.floor(i / cols.current)
        const c = i % cols.current
        const dr = r - centerRow
        const dc = c - centerCol
        const dist = Math.sqrt(dr * dr + dc * dc)
        if (dist > SPOT_RADIUS) {
          // only restore if it was spotlight-lit (avoid mass resets every move)
          const cell = cells.current[i]
          if (
            cell &&
            (cell.bRT !== borderRgb.current[0] ||
              cell.bGT !== borderRgb.current[1] ||
              cell.bBT !== borderRgb.current[2])
          ) {
            if (!rEggCells.current.has(i)) {
              setBorderTarget(
                i,
                borderRgb.current[0],
                borderRgb.current[1],
                borderRgb.current[2],
                speedFromFrames(18)
              )
            }
          }
        } else {
          const t = Math.pow(1 - dist / SPOT_RADIUS, 1.5)
          const tr = Math.round(borderRgb.current[0] * (1 - t) + accentRgb.current[0] * t)
          const tg = Math.round(borderRgb.current[1] * (1 - t) + accentRgb.current[1] * t)
          const tb = Math.round(borderRgb.current[2] * (1 - t) + accentRgb.current[2] * t)
          if (!rEggCells.current.has(i)) {
            setBorderTarget(i, tr, tg, tb, speedFromFrames(7))
          }
        }
      }
    }

    const handleClick = (e: MouseEvent) => {
      const rect = container.getBoundingClientRect()
      const sz = cellSize.current
      if (sz === 0) return
      const inside =
        e.clientX >= rect.left && e.clientX <= rect.right && e.clientY >= rect.top && e.clientY <= rect.bottom
      if (!inside) return

      cancelREgg()

      const col = Math.max(0, Math.min(Math.floor((e.clientX - rect.left) / (sz + GAP)), cols.current - 1))
      const row = Math.max(0, Math.min(Math.floor((e.clientY - rect.top) / (sz + GAP)), rows.current - 1))
      const startCol = Math.max(0, Math.min(col - 1, cols.current - 3))
      const startRow = Math.max(0, Math.min(row - 1, rows.current - 3))

      const rIndices: number[] = []
      for (let dr = 0; dr < 3; dr++) {
        for (let dc = 0; dc < 3; dc++) {
          rIndices.push((startRow + dr) * cols.current + (startCol + dc))
        }
      }

      rIndices.forEach((idx) => {
        const cell = cells.current[idx]
        if (!cell) return
        const sz2 = cell.size
        if (!hoverActive.current.has(idx)) {
          preHoverShape.current[idx] = {
            tl: cell.tlT / sz2,
            tr: cell.trT / sz2,
            br: cell.brT / sz2,
            bl: cell.blT / sz2
          }
        }
        hoverActive.current.add(idx)
        rEggCells.current.add(idx)
      })

      rIndices.forEach((idx, i) => {
        staggerTimers.current.push(
          setTimeout(() => {
            setTarget(idx, R_EGG_SHAPES[i], speedFromFrames(30))
            setBorderTarget(
              idx,
              accentRgb.current[0],
              accentRgb.current[1],
              accentRgb.current[2],
              speedFromFrames(18)
            )
          }, i * 55)
        )
      })

      rEggTimer.current = setTimeout(() => {
        rIndices.forEach((idx, i) => {
          setTimeout(() => {
            const restore = preHoverShape.current[idx]
            if (restore) setTarget(idx, restore, speedFromFrames(36))
            setBorderTarget(
              idx,
              borderRgb.current[0],
              borderRgb.current[1],
              borderRgb.current[2],
              speedFromFrames(30)
            )
            hoverActive.current.delete(idx)
            rEggCells.current.delete(idx)
          }, i * 40)
        })
      }, R_HOLD_MS)
    }

    document.addEventListener('mousemove', handleMove, { passive: true })
    document.addEventListener('click', handleClick)
    return () => {
      document.removeEventListener('mousemove', handleMove)
      document.removeEventListener('click', handleClick)
      if (rEggTimer.current) clearTimeout(rEggTimer.current)
      staggerTimers.current.forEach(clearTimeout)
    }
  }, [spotlight])

  return (
    <div
      ref={containerRef}
      className="pointer-events-none absolute inset-0 z-0 overflow-hidden"
      style={{ background: 'transparent' }}
    >
      <canvas ref={canvasRef} style={{ display: 'block' }} />
    </div>
  )
}
