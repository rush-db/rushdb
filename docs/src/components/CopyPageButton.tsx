import React, { useCallback, useEffect, useRef, useState } from 'react'
import BrowserOnly from '@docusaurus/BrowserOnly'
import { useLocation } from '@docusaurus/router'
import { Clipboard, Check, ChevronDown, FileText, Maximize2, Minimize2 } from 'lucide-react'

const GITHUB_RAW = 'https://raw.githubusercontent.com/rush-db/rushdb/main/docs/docs'
const DOCS_BASE = 'https://docs.rushdb.com'

function ClaudeIcon() {
  return (
    <svg
      version="1.1"
      id="Layer_1"
      xmlns="http://www.w3.org/2000/svg"
      x="0px"
      y="0px"
      viewBox="0 0 92.2 65"
      xmlSpace="preserve"
      width="20"
      height="20"
    >
      <path
        fill="currentColor"
        d="M66.5,0H52.4l25.7,65h14.1L66.5,0z M25.7,0L0,65h14.4l5.3-13.6h26.9L51.8,65h14.4L40.5,0C40.5,0,25.7,0,25.7,0z
	 M24.3,39.3l8.8-22.8l8.8,22.8H24.3z"
      ></path>
    </svg>
  )
}

function ChatGPTIcon() {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 320 320">
      <path
        fill="currentColor"
        d="m297.06 130.97c7.26-21.79 4.76-45.66-6.85-65.48-17.46-30.4-52.56-46.04-86.84-38.68-15.25-17.18-37.16-26.95-60.13-26.81-35.04-.08-66.13 22.48-76.91 55.82-22.51 4.61-41.94 18.7-53.31 38.67-17.59 30.32-13.58 68.54 9.92 94.54-7.26 21.79-4.76 45.66 6.85 65.48 17.46 30.4 52.56 46.04 86.84 38.68 15.24 17.18 37.16 26.95 60.13 26.8 35.06.09 66.16-22.49 76.94-55.86 22.51-4.61 41.94-18.7 53.31-38.67 17.57-30.32 13.55-68.51-9.94-94.51zm-120.28 168.11c-14.03.02-27.62-4.89-38.39-13.88.49-.26 1.34-.73 1.89-1.07l63.72-36.8c3.26-1.85 5.26-5.32 5.24-9.07v-89.83l26.93 15.55c.29.14.48.42.52.74v74.39c-.04 33.08-26.83 59.9-59.91 59.97zm-128.84-55.03c-7.03-12.14-9.56-26.37-7.15-40.18.47.28 1.3.79 1.89 1.13l63.72 36.8c3.23 1.89 7.23 1.89 10.47 0l77.79-44.92v31.1c.02.32-.13.63-.38.83l-64.41 37.19c-28.69 16.52-65.33 6.7-81.92-21.95zm-16.77-139.09c7-12.16 18.05-21.46 31.21-26.29 0 .55-.03 1.52-.03 2.2v73.61c-.02 3.74 1.98 7.21 5.23 9.06l77.79 44.91-26.93 15.55c-.27.18-.61.21-.91.08l-64.42-37.22c-28.63-16.58-38.45-53.21-21.95-81.89zm221.26 51.49-77.79-44.92 26.93-15.54c.27-.18.61-.21.91-.08l64.42 37.19c28.68 16.57 38.51 53.26 21.94 81.94-7.01 12.14-18.05 21.44-31.2 26.28v-75.81c.03-3.74-1.96-7.2-5.2-9.06zm26.8-40.34c-.47-.29-1.3-.79-1.89-1.13l-63.72-36.8c-3.23-1.89-7.23-1.89-10.47 0l-77.79 44.92v-31.1c-.02-.32.13-.63.38-.83l64.41-37.16c28.69-16.55 65.37-6.7 81.91 22 6.99 12.12 9.52 26.31 7.15 40.1zm-168.51 55.43-26.94-15.55c-.29-.14-.48-.42-.52-.74v-74.39c.02-33.12 26.89-59.96 60.01-59.94 14.01 0 27.57 4.92 38.34 13.88-.49.26-1.33.73-1.89 1.07l-63.72 36.8c-3.26 1.85-5.26 5.31-5.24 9.06l-.04 89.79zm14.63-31.54 34.65-20.01 34.65 20v40.01l-34.65 20-34.65-20z"
      />
    </svg>
  )
}

function T3Icon() {
  return (
    <svg viewBox="0 0 258 199" fill="currentColor" width="20" height="20" xmlns="http://www.w3.org/2000/svg">
      <path
        fill-rule="evenodd"
        clip-rule="evenodd"
        d="M165.735 25.0701L188.947 0.972412H0.465994V25.0701H165.735Z"
      ></path>
      <path d="M163.981 96.3239L254.022 3.68314L221.206 3.68295L145.617 80.7609L163.981 96.3239Z"></path>
      <path d="M233.658 131.418C233.658 155.075 214.48 174.254 190.823 174.254C171.715 174.254 155.513 161.738 150 144.439L146.625 133.848L127.329 153.143L129.092 157.336C139.215 181.421 163.034 198.354 190.823 198.354C227.791 198.354 257.759 168.386 257.759 131.418C257.759 106.937 244.399 85.7396 224.956 74.0905L220.395 71.3582L202.727 89.2528L210.788 93.5083C224.403 100.696 233.658 114.981 233.658 131.418Z"></path>
      <path
        fill-rule="evenodd"
        clip-rule="evenodd"
        d="M88.2625 192.669L88.2626 45.6459H64.1648L64.1648 192.669H88.2625Z"
      ></path>
    </svg>
  )
}

function MarkdownIcon() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      width="20"
      height="20"
      xmlns="http://www.w3.org/2000/svg"
    >
      <rect x="3" y="5" width="18" height="14" rx="2"></rect>
      <path d="M7 15V9l2 2 2-2v6"></path>
      <path d="M17 9v6l-2-2"></path>
    </svg>
  )
}

interface MenuItem {
  key: string
  Icon: React.ComponentType
  label: string
  description: string
  getHref: (pageUrl: string, markdownUrl: string) => string
}

const LLM_PROMPT = (url: string) => `Read from this URL: ${url} and explain it to me`

const MENU_ITEMS: MenuItem[] = [
  {
    key: 'markdown',
    Icon: MarkdownIcon,
    label: 'View as Markdown',
    description: 'Open this page in Markdown',
    getHref: (_pageUrl, markdownUrl) => markdownUrl
  },
  {
    key: 'claude',
    Icon: ClaudeIcon,
    label: 'Open in Claude',
    description: 'Ask questions about this page',
    getHref: (pageUrl) => `https://claude.ai/new?q=${encodeURIComponent(LLM_PROMPT(pageUrl))}`
  },
  {
    key: 'chatgpt',
    Icon: ChatGPTIcon,
    label: 'Open in ChatGPT',
    description: 'Ask questions about this page',
    getHref: (pageUrl) => `https://chatgpt.com/?q=${encodeURIComponent(LLM_PROMPT(pageUrl))}`
  },
  {
    key: 't3',
    Icon: T3Icon,
    label: 'Open in T3 Chat',
    description: 'Ask questions about this page',
    getHref: (pageUrl) => `https://t3.chat/new?q=${encodeURIComponent(LLM_PROMPT(pageUrl))}`
  }
]

function CopyPageButtonInner() {
  const [open, setOpen] = useState(false)
  const [copied, setCopied] = useState(false)
  const [resolvedMarkdownUrl, setResolvedMarkdownUrl] = useState<string | null>(null)
  const containerRef = useRef<HTMLDivElement>(null)
  const location = useLocation()

  const slug = location.pathname.replace(/\/+$/, '').replace(/^\//, '') || 'index'
  const pageUrl = slug === 'index' ? DOCS_BASE : `${DOCS_BASE}/${slug}`
  const markdownUrl = `${GITHUB_RAW}/${slug}.md`
  const markdownUrlMdx = `${GITHUB_RAW}/${slug}.mdx`

  // Resolve which extension actually exists; cache per slug
  useEffect(() => {
    setResolvedMarkdownUrl(null)
    fetch(markdownUrl, { method: 'HEAD' })
      .then((r) => {
        if (r.ok) {
          setResolvedMarkdownUrl(markdownUrl)
          return
        }
        fetch(markdownUrlMdx, { method: 'HEAD' })
          .then((r2) => {
            setResolvedMarkdownUrl(r2.ok ? markdownUrlMdx : markdownUrl)
          })
          .catch(() => setResolvedMarkdownUrl(markdownUrl))
      })
      .catch(() => setResolvedMarkdownUrl(markdownUrl))
  }, [slug])

  const effectiveMarkdownUrl = resolvedMarkdownUrl ?? markdownUrl

  const handleCopy = useCallback(async () => {
    try {
      const res = await fetch(effectiveMarkdownUrl)
      const text = res.ok ? await res.text() : null
      const content = text ?? `# ${document.title}\n\n${window.location.href}`
      await navigator.clipboard.writeText(content)
    } catch {
      // fallback: copy the URL when fetch/clipboard fails
      try {
        await navigator.clipboard.writeText(window.location.href)
      } catch {
        const el = document.createElement('textarea')
        el.value = window.location.href
        document.body.appendChild(el)
        el.select()
        document.execCommand('copy')
        document.body.removeChild(el)
      }
    }
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }, [effectiveMarkdownUrl])

  useEffect(() => {
    if (!open) return
    const onMouseDown = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false)
      }
    }
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false)
    }
    document.addEventListener('mousedown', onMouseDown)
    document.addEventListener('keydown', onKeyDown)
    return () => {
      document.removeEventListener('mousedown', onMouseDown)
      document.removeEventListener('keydown', onKeyDown)
    }
  }, [open])

  return (
    <div ref={containerRef} className="copy-page-container">
      {/* Main copy button */}
      <button className="copy-page-main-btn" onClick={handleCopy} title="Copy page URL to clipboard">
        {copied ?
          <Check size={13} strokeWidth={2.5} />
        : <Clipboard size={13} strokeWidth={2} />}
        <span>{copied ? 'Copied!' : 'Copy page'}</span>
      </button>

      {/* Dropdown chevron */}
      <button
        className="copy-page-chevron-btn"
        onClick={() => setOpen((v) => !v)}
        aria-label="More options"
        aria-expanded={open}
        aria-haspopup="menu"
      >
        <ChevronDown size={11} strokeWidth={2.5} />
      </button>

      {/* Dropdown menu */}
      {open && (
        <div className="copy-page-dropdown" role="menu">
          {MENU_ITEMS.map((item, idx) => (
            <a
              key={item.key}
              href={item.getHref(pageUrl, effectiveMarkdownUrl)}
              target="_blank"
              rel="noopener noreferrer"
              role="menuitem"
              className={`copy-page-item ${idx > 0 ? 'copy-page-item--bordered' : ''}`}
              onClick={() => setOpen(false)}
            >
              <span className="copy-page-item-icon">
                <item.Icon />
              </span>
              <span className="copy-page-item-text">
                <span className="copy-page-item-label">{item.label}</span>
                <span className="copy-page-item-desc">{item.description}</span>
              </span>
            </a>
          ))}
        </div>
      )}
    </div>
  )
}

// ── Public export ───────────────────────────────────────────────────────────

export default function CopyPageButton() {
  return <BrowserOnly fallback={<div style={{ height: 29 }} />}>{() => <CopyPageButtonInner />}</BrowserOnly>
}

// ── Widescreen toggle ────────────────────────────────────────────────────────

const WIDESCREEN_KEY = 'rushdb-docs-widescreen'

function WidescreenButtonInner() {
  const [wide, setWide] = useState(false)

  useEffect(() => {
    const stored = localStorage.getItem(WIDESCREEN_KEY) === 'true'
    setWide(stored)
    if (stored) document.documentElement.setAttribute('data-widescreen', 'true')
  }, [])

  const toggle = useCallback(() => {
    setWide((v) => {
      const next = !v
      localStorage.setItem(WIDESCREEN_KEY, String(next))
      if (next) {
        document.documentElement.setAttribute('data-widescreen', 'true')
      } else {
        document.documentElement.removeAttribute('data-widescreen')
      }
      return next
    })
  }, [])

  return (
    <button
      className={`widescreen-btn`}
      onClick={toggle}
      title={wide ? 'Exit widescreen' : 'Widescreen mode'}
      aria-pressed={wide}
    >
      {wide ?
        <Minimize2 size={13} strokeWidth={2} />
      : <Maximize2 size={13} strokeWidth={2} />}
    </button>
  )
}

export function WidescreenButton() {
  return (
    <BrowserOnly fallback={<div style={{ width: 29, height: 29 }} />}>
      {() => <WidescreenButtonInner />}
    </BrowserOnly>
  )
}
