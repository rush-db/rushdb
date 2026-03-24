import React from 'react'
import Tabs from '@theme/Tabs'

// ── Icons ──────────────────────────────────────────────────────────────────

function PythonIcon(): React.ReactElement {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 48 48"
      width="16"
      height="16"
      style={{ verticalAlign: 'text-bottom', marginRight: 5, flexShrink: 0 }}
      aria-hidden="true"
    >
      <path
        fill="#3776AB"
        d="M24.046 5C13.613 5 14.25 9.707 14.25 9.707l.012 4.857h9.965v1.457H11.024S5 14.317 5 24.815c0 10.499 5.81 10.127 5.81 10.127h3.467v-4.873s-.187-5.81 5.713-5.81h9.845s5.529.09 5.529-5.345V11.015S36.162 5 24.046 5zm-5.486 3.174c.993 0 1.8.807 1.8 1.8s-.807 1.8-1.8 1.8-1.8-.807-1.8-1.8.807-1.8 1.8-1.8z"
      />
      <path
        fill="#FFD43B"
        d="M24.24 43c10.433 0 9.796-4.707 9.796-4.707l-.012-4.857h-9.965v-1.457H37.26S43.286 33.683 43.286 23.185c0-10.499-5.81-10.127-5.81-10.127h-3.467v4.873s.187 5.81-5.713 5.81h-9.845s-5.529-.09-5.529 5.345v8.918S12.124 43 24.24 43zm5.486-3.174c-.993 0-1.8-.807-1.8-1.8s.807-1.8 1.8-1.8 1.8.807 1.8 1.8-.807 1.8-1.8 1.8z"
      />
    </svg>
  )
}

function TypeScriptIcon(): React.ReactElement {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      width="16"
      height="16"
      style={{ verticalAlign: 'text-bottom', marginRight: 5, flexShrink: 0 }}
      aria-hidden="true"
    >
      <rect fill="#3178C6" width="22" height="22" x="1" y="1" rx="3.5" />
      <text
        x="12"
        y="17"
        textAnchor="middle"
        fill="white"
        fontFamily="'Segoe UI','Helvetica Neue',Arial,sans-serif"
        fontSize="11"
        fontWeight="700"
        letterSpacing="-0.5"
      >
        TS
      </text>
    </svg>
  )
}

function ShellIcon(): React.ReactElement {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      width="16"
      height="16"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      style={{ verticalAlign: 'text-bottom', marginRight: 5, flexShrink: 0 }}
      aria-hidden="true"
    >
      <polyline points="4 17 10 11 4 5" />
      <line x1="12" y1="19" x2="20" y2="19" />
    </svg>
  )
}

// ── Language config ────────────────────────────────────────────────────────

/** Render order: lower number = shown first. Unknown values get 99. */
const LANG_ORDER: Record<string, number> = {
  python: 0,
  typescript: 1,
  shell: 2
}

const LANG_CONFIG: Record<string, { label: string; Icon: () => React.ReactElement }> = {
  python: { label: 'Python', Icon: PythonIcon },
  typescript: { label: 'TypeScript', Icon: TypeScriptIcon },
  shell: { label: 'shell', Icon: ShellIcon }
}

// ── Component ──────────────────────────────────────────────────────────────

interface LanguageTabsProps {
  children: React.ReactNode
  /** Accepted for drop-in compatibility with <Tabs groupId="..."> usage; always
   *  overridden internally to "programming-language" for cross-page tab syncing. */
  groupId?: string
  [key: string]: unknown
}

/**
 * Drop-in replacement for `<Tabs groupId="programming-language">` that:
 *  - Enforces `groupId="programming-language"` (cross-page tab syncing)
 *  - Always renders Python first, TypeScript second, anything else after
 *  - Injects the Python 🐍 and TypeScript TS badge icons into tab labels
 *  - Sets Python as the default active tab
 *
 * Usage in MDX (import as `Tabs` to avoid changing any body tags):
 * ```mdx
 * import Tabs from '@site/src/components/LanguageTabs';
 * import TabItem from '@theme/TabItem';
 * ```
 */
export default function LanguageTabs({ children }: LanguageTabsProps): React.ReactElement {
  const items = React.Children.toArray(children).filter((child): child is React.ReactElement =>
    React.isValidElement(child)
  )

  const sorted = [...items].sort(
    // @ts-ignore
    (a, b) => (LANG_ORDER[a.props.value] ?? 99) - (LANG_ORDER[b.props.value] ?? 99)
  )

  const enhanced = sorted.map((child, index) => {
    // @ts-ignore
    const value: string = child.props.value
    const config = LANG_CONFIG[value]

    const label =
      config ?
        <span style={{ display: 'inline-flex', alignItems: 'center', gap: 5 }}>
          <config.Icon />
          {config.label}
        </span>
        // @ts-ignore
      : child.props.label

    return React.cloneElement(child as React.ReactElement<Record<string, unknown>>, {
      key: value,
      label,
      default: index === 0
    })
  })

  // @ts-ignore
  return (
    <div className="language-tabs">
      <Tabs groupId="programming-language">{enhanced}</Tabs>
    </div>
  )
}
