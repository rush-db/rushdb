import { useEffect, useRef, useState } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { materialDark, materialLight } from 'react-syntax-highlighter/dist/cjs/styles/prism'
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter'

function makeSyntaxOverride(isDark: boolean) {
  const base = isDark ? materialDark : materialLight
  const bg = 'transparent'
  return {
    ...base,
    'code[class*="language-"]': {
      ...base['code[class*="language-"]'],
      background: bg,
      fontFamily: 'var(--font-jet-brains-mono)',
      fontSize: '12px',
      lineHeight: '1.6',
      color: isDark ? '#e0e0d8' : '#2a2520'
    },
    'pre[class*="language-"]': {
      ...base['pre[class*="language-"]'],
      background: bg,
      margin: 0,
      padding: 0,
      overflow: 'hidden'
    },
    // warm orange strings in light, neon in dark
    string: { color: isDark ? '#00FF85' : '#C8540A' },
    number: { color: isDark ? '#82aaff' : '#1d6fa5' },
    boolean: { color: isDark ? '#f78c6c' : '#a03000' },
    null: { color: isDark ? '#6b6b72' : '#7a7268' },
    property: { color: isDark ? '#b0bec5' : '#4a4540' },
    punctuation: { color: isDark ? '#546e7a' : '#9a9490' }
  }
}

const FRAMES = [
  {
    json: `{
  "agent_id": "researcher-01",
  "session": "sess_9c3f1a",
  "step": 3,
  "tool": "web_search",
  "query": "top open-source LLMs 2025",
  "result": "Llama 4 leads reasoning at 94.2...",
  "tokens": 812,
  "embedding": "auto"
}`
  },
  {
    json: `{
  "doc_id": "kb_arch_042",
  "title": "API Design Guidelines",
  "owner": { "id": "u_maya_r", "team": "platform" },
  "tags": ["api", "backend", "standards"],
  "chunks": 18,
  "project": "core-infra",
  "embedding": "auto"
}`
  },
  {
    json: `{
  "meeting_id": "meet_q4_kick",
  "topic": "Q4 roadmap priorities",
  "attendees": ["maya_r", "jack_t", "priya_k"],
  "decisions": ["ship graph RAG", "drop Redis"],
  "action_items": 6,
  "project": "horizon-q4",
  "embedding": "auto"
}`
  }
]

// Per-frame graph topologies — some nodes intentionally beyond the SVG viewport edge
const FRAME_GRAPHS: Array<{
  nodes: Array<{ x: number; y: number }>
  edges: Array<[number, number]>
  labels: string[]
}> = [
  {
    // Frame 0: Star/hub — agent at center, 8 spokes radiating out, a few clipped
    nodes: [
      { x: 280, y: 110 }, // 0 AGENT
      { x: 90, y: 22 }, // 1 SESSION
      { x: 280, y: 10 }, // 2 ACTION  ← near top edge
      { x: 470, y: 22 }, // 3 QUERY
      { x: 520, y: 110 }, // 4 RESULT  ← beyond right
      { x: 470, y: 198 }, // 5 FILTER
      { x: 280, y: 218 }, // 6 SCORE   ← near bottom
      { x: 90, y: 198 }, // 7 LATENCY
      { x: -20, y: 110 } // 8 INDEX   ← beyond left
    ],
    edges: [
      [0, 1],
      [0, 2],
      [0, 3],
      [0, 4],
      [0, 5],
      [0, 6],
      [0, 7],
      [0, 8]
    ],
    labels: ['AGENT', 'SESSION', 'STEP', 'TOOL', 'QUERY', 'RESULT', 'TOKENS', 'EMBED', 'INDEX']
  },
  {
    // Frame 1: Tree — doc root cascades down hierarchically
    nodes: [
      { x: 280, y: 8 }, // 0 DOC     ← near top
      { x: 90, y: 100 }, // 1 AUTHOR
      { x: 280, y: 95 }, // 2 CONTENT
      { x: 470, y: 100 }, // 3 TAGS
      { x: -15, y: 200 }, // 4 ID      ← beyond left
      { x: 130, y: 215 }, // 5 ROLE    ← near bottom
      { x: 330, y: 210 }, // 6 SECTION
      { x: 570, y: 200 } // 7 INDEX   ← beyond right
    ],
    edges: [
      [0, 1],
      [0, 2],
      [0, 3],
      [1, 4],
      [1, 5],
      [2, 6],
      [3, 6],
      [3, 7]
    ],
    labels: ['DOC', 'OWNER', 'CONTENT', 'TAGS', 'ID', 'TEAM', 'CHUNK', 'INDEX']
  },
  {
    // Frame 2: Pipeline chain left-to-right with satellite nodes above/below
    nodes: [
      { x: -15, y: 115 }, // 0 USER    ← beyond left
      { x: 135, y: 115 }, // 1 EVENT
      { x: 280, y: 115 }, // 2 PLAN
      { x: 425, y: 115 }, // 3 META
      { x: 575, y: 115 }, // 4 INDEX   ← beyond right
      { x: 135, y: 8 }, // 5 COUPON  ← near top
      { x: 280, y: 12 }, // 6 AMOUNT
      { x: 425, y: 8 }, // 7 TRIAL   ← near top
      { x: 280, y: 222 } // 8 SOURCE  ← beyond bottom
    ],
    edges: [
      [0, 1],
      [1, 2],
      [2, 3],
      [3, 4],
      [1, 5],
      [2, 6],
      [3, 7],
      [2, 8]
    ],
    labels: ['MEETING', 'ATTEND', 'TOPIC', 'PROJECT', 'INDEX', 'DECISION', 'ACTION', 'ITEM', 'SOURCE']
  }
]

// Generate seeded-ish vector matrix rows
function makeVecRow(seed: number, cols: number): string {
  const nums: string[] = []
  let s = seed
  for (let i = 0; i < cols; i++) {
    s = (s * 1664525 + 1013904223) & 0xffffffff
    const v = ((s >>> 16) & 0xffff) / 65535
    nums.push((v * 2 - 1).toFixed(4))
  }
  return nums.join('  ')
}

const VEC_ROWS = 6
const VEC_COLS = 6
const VEC_LINES = Array.from({ length: VEC_ROWS }, (_, i) => makeVecRow(i * 7919 + 42, VEC_COLS))

type Phase = 'typing' | 'transforming' | 'showing' | 'resetting'

// Renders syntax-highlighted typed text
function TypedCode({ text, isDark }: { text: string; isDark: boolean }) {
  const override = makeSyntaxOverride(isDark)
  const fontStyle: React.CSSProperties = {
    fontFamily: 'var(--font-jet-brains-mono)',
    fontSize: '12px',
    lineHeight: '1.6',
    whiteSpace: 'pre'
  }

  return (
    <SyntaxHighlighter
      language="json"
      style={override}
      customStyle={{ background: 'transparent', padding: 0, margin: 0, overflow: 'hidden' }}
      codeTagProps={{ style: fontStyle }}
    >
      {text || ' '}
    </SyntaxHighlighter>
  )
}

// Animated vector matrix — numbers stream in rapidly
function VectorMatrix({ show, isDark }: { show: boolean; isDark: boolean }) {
  const [revealed, setRevealed] = useState(0)
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)

  useEffect(() => {
    if (timerRef.current) clearInterval(timerRef.current)
    if (!show) {
      setRevealed(0)
      return
    }
    setRevealed(0)
    let r = 0
    timerRef.current = setInterval(() => {
      r++
      setRevealed(r)
      if (r >= VEC_ROWS) clearInterval(timerRef.current!)
    }, 80)
    return () => {
      if (timerRef.current) clearInterval(timerRef.current)
    }
  }, [show])

  const dimColor = isDark ? 'rgba(0,255,133,0.25)' : 'rgba(200,84,10,0.30)'
  const brightColor = isDark ? 'rgba(0,255,133,0.75)' : 'rgba(200,84,10,0.85)'
  const labelColor = isDark ? 'rgba(240,240,238,0.35)' : 'rgba(28,26,22,0.35)'

  return (
    <div
      style={{
        fontFamily: 'var(--font-jet-brains-mono)',
        fontSize: '10px',
        lineHeight: '1.7',
        letterSpacing: '0.02em'
      }}
    >
      <div style={{ color: labelColor, marginBottom: 4 }}>
        embedding[0..{VEC_ROWS - 1}] · dim={VEC_COLS * 8}
      </div>
      {VEC_LINES.map((line, row) => (
        <div
          key={row}
          style={{
            opacity: row < revealed ? 1 : 0,
            transition: 'opacity 0.12s',
            whiteSpace: 'nowrap',
            overflow: 'hidden'
          }}
        >
          <span style={{ color: labelColor, userSelect: 'none' }}>{String(row).padStart(2, '0')}│ </span>
          {line.split('  ').map((num, ci) => (
            <span
              key={ci}
              style={{
                color: Math.abs(parseFloat(num)) > 0.7 ? brightColor : dimColor,
                marginRight: '1ch'
              }}
            >
              {num}
            </span>
          ))}
        </div>
      ))}
    </div>
  )
}

export function AnimatedJSON() {
  const [frame, setFrame] = useState(0)
  const [phase, setPhase] = useState<Phase>('typing')
  const [text, setText] = useState('')
  const [showOutput, setShowOutput] = useState(false)
  const [isDark, setIsDark] = useState(true)

  // Track theme
  useEffect(() => {
    const sync = () => setIsDark(document.documentElement.classList.contains('dark'))
    sync()
    const mo = new MutationObserver(sync)
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
    return () => mo.disconnect()
  }, [])

  useEffect(() => {
    const fullJson = FRAMES[frame].json
    let i = 0
    setText('')
    setShowOutput(false)
    setPhase('typing')

    const timeouts: ReturnType<typeof setTimeout>[] = []

    const timer = setInterval(() => {
      i++
      setText(fullJson.slice(0, i))
      if (i >= fullJson.length) {
        clearInterval(timer)
        timeouts.push(setTimeout(() => setPhase('transforming'), 150))
        timeouts.push(
          setTimeout(() => {
            setPhase('showing')
            setShowOutput(true)
          }, 500)
        )
        timeouts.push(
          setTimeout(() => {
            setPhase('resetting')
            setShowOutput(false)
          }, 3200)
        )
        timeouts.push(setTimeout(() => setFrame((f) => (f + 1) % FRAMES.length), 3600))
      }
    }, 8)

    return () => {
      clearInterval(timer)
      timeouts.forEach(clearTimeout)
    }
  }, [frame])

  const { nodes: frameNodes, edges: frameEdges, labels } = FRAME_GRAPHS[frame]
  const isTransforming = phase === 'transforming'

  // Theme-derived inline colours
  const panelBg = isDark ? '#0f0f11' : 'var(--lp-surface)'
  const inputBg = isDark ? '#131313' : 'var(--lp-bg)'
  const shadow =
    isDark ?
      '0 0 0 1px #1E1E22, 0 40px 100px rgba(0,0,0,0.7), 0 0 60px rgba(0,255,133,0.05)'
    : '0 0 0 1px var(--lp-border), 0 8px 40px rgba(0,0,0,0.10)'
  const nodeCircleFill = isDark ? '#111113' : 'var(--lp-surface)'
  const nodeStroke = 'var(--lp-accent)'
  const edgeStroke = 'var(--lp-accent)'
  const nodeTextFill = isDark ? '#F0F0EE' : '#1C1A16'

  return (
    <div
      className="border-lp-border h-[400px] w-full overflow-hidden border sm:h-auto"
      style={{ background: panelBg, boxShadow: shadow }}
    >
      <div className="flex h-full flex-row items-stretch sm:flex-col">
        {/* Left — JSON input */}
        <div
          className="border-lp-border flex w-2/5 flex-col border-r sm:w-full sm:shrink-0 sm:border-b sm:border-r-0"
          style={{ background: inputBg }}
        >
          <div className="border-lp-border flex items-center gap-2 border-b px-4 py-2 sm:px-2">
            <span className="bg-lp-border h-2 w-2 rounded-full" />
            <span className="text-lp-muted font-mono text-sm uppercase tracking-widest">json input</span>
          </div>
          <div className="flex-1 overflow-hidden p-5 sm:h-[155px] sm:flex-none sm:p-2">
            <TypedCode text={text} isDark={isDark} />
          </div>
        </div>

        {/* Center — push arrow */}
        <div
          className="border-lp-border flex shrink-0 flex-col items-center justify-center gap-2 border-r px-5 sm:flex-row sm:border-b sm:border-r-0 sm:px-3 sm:py-2"
          style={{ background: panelBg }}
        >
          <motion.span
            className="text-lp-accent font-mono text-sm uppercase tracking-widest"
            animate={{ opacity: isTransforming ? [0.3, 1, 0.3] : 0.35 }}
            transition={{ repeat: isTransforming ? Infinity : 0, duration: 0.4 }}
          >
            push
          </motion.span>
          <motion.span
            className="text-lp-accent text-sm leading-none"
            animate={{ x: isTransforming ? [0, 5, 0] : 0 }}
            transition={{ repeat: isTransforming ? Infinity : 0, duration: 0.4 }}
          >
            →
          </motion.span>
        </div>

        {/* Right — graph + vectors */}
        <div className="flex flex-1 flex-col">
          {/* Graph panel */}
          <div className="border-lp-border flex-1 border-b">
            <div className="border-lp-border flex items-center gap-2 border-b px-4 py-2 sm:px-2">
              <span className="bg-lp-border h-2 w-2 rounded-full" />
              <span className="text-lp-muted font-mono text-sm uppercase tracking-widest">
                graph — auto-linked nodes
              </span>
            </div>
            <div className="p-3 sm:p-1">
              <svg
                key={frame}
                viewBox="0 0 560 220"
                className="h-[140px] w-full"
                style={{ overflow: 'visible' }}
              >
                <defs>
                  <filter id="glow">
                    <feGaussianBlur stdDeviation="2" result="blur" />
                    <feMerge>
                      <feMergeNode in="blur" />
                      <feMergeNode in="SourceGraphic" />
                    </feMerge>
                  </filter>
                </defs>
                {frameEdges.map(([from, to], i) => {
                  const a = frameNodes[from]
                  const b = frameNodes[to]
                  const len = Math.hypot(b.x - a.x, b.y - a.y)
                  return (
                    <motion.line
                      key={i}
                      x1={a.x}
                      y1={a.y}
                      x2={b.x}
                      y2={b.y}
                      stroke={edgeStroke}
                      strokeWidth={1}
                      strokeOpacity={0.3}
                      strokeDasharray={len}
                      initial={{ strokeDashoffset: len }}
                      animate={{ strokeDashoffset: showOutput ? 0 : len }}
                      transition={{ duration: 0.4, delay: i * 0.07 }}
                    />
                  )
                })}
                {frameNodes.map((pos, i) => (
                  <motion.g key={i} filter="url(#glow)">
                    <motion.circle
                      cx={pos.x}
                      cy={pos.y}
                      fill={nodeCircleFill}
                      stroke={nodeStroke}
                      strokeWidth={1}
                      strokeOpacity={0.8}
                      initial={{ r: 0, opacity: 0 }}
                      animate={{ r: showOutput ? 18 : 0, opacity: showOutput ? 1 : 0 }}
                      transition={{ duration: 0.3, delay: i * 0.08 }}
                    />
                    <motion.text
                      x={pos.x}
                      y={pos.y + 4}
                      textAnchor="middle"
                      fill={nodeTextFill}
                      fontSize={7}
                      fontFamily="monospace"
                      initial={{ opacity: 0 }}
                      animate={{ opacity: showOutput ? 0.9 : 0 }}
                      transition={{ duration: 0.2, delay: 0.12 + i * 0.08 }}
                    >
                      {labels[i] ?? ''}
                    </motion.text>
                  </motion.g>
                ))}
              </svg>
            </div>
          </div>

          {/* Vector panel */}
          <div>
            <div className="border-lp-border flex items-center gap-2 border-b px-4 py-2 sm:px-2">
              <span className="bg-lp-border h-2 w-2 rounded-full" />
              <span className="text-lp-muted font-mono text-sm uppercase tracking-widest">
                vector embeddings
              </span>
            </div>
            <div className="h-[120px] overflow-hidden p-3 sm:p-2">
              <AnimatePresence>
                {showOutput && (
                  <motion.div
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    exit={{ opacity: 0 }}
                    transition={{ duration: 0.2 }}
                  >
                    <VectorMatrix show={showOutput} isDark={isDark} />
                  </motion.div>
                )}
              </AnimatePresence>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
