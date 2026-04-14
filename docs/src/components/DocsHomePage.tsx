import React from 'react'
import {
  ArrowRight,
  Book,
  Check,
  Globe,
  Monitor,
  Rocket,
  Share2,
  Shield,
  Sparkles,
  Sun,
  Upload
} from 'lucide-react'

const BORDER_CLASS = 'border-[var(--ifm-color-emphasis-200)]'

// ── Icons ─────────────────────────────────────────────────────────────────────

const TypeScriptIcon = () => (
  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" aria-hidden="true">
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

const PythonIcon = () => (
  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48" width="24" height="24" aria-hidden="true">
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

// ── Data ──────────────────────────────────────────────────────────────────────

type InterfaceCard = {
  icon: () => React.ReactElement
  label: string
  badge: string
  badgeColor: string
  description: string
  features: string[]
  href: string
}

const INTERFACE_CARDS: InterfaceCard[] = [
  {
    icon: TypeScriptIcon,
    label: 'TypeScript SDK',
    badge: 'npm',
    badgeColor: '#3178C6',
    description: 'Full type safety for Node.js and browsers. Async/await API with zero config.',
    features: ['Type-safe query builder', 'Browser + Node.js', 'ESM & CommonJS'],
    href: '/typescript-sdk/introduction'
  },
  {
    icon: PythonIcon,
    label: 'Python SDK',
    badge: 'pip',
    badgeColor: '#3776AB',
    description: 'Ergonomic client for backend scripts and data workflows. Sync and async.',
    features: ['Sync & async client', 'Pandas-friendly output', 'Pipeline-ready'],
    href: '/python-sdk/introduction'
  },
  {
    icon: () => <Globe size={24} strokeWidth={1.5} />,
    label: 'REST API',
    badge: 'HTTP',
    badgeColor: '#16a34a',
    description: 'Language-agnostic HTTP access. Works from any stack with curl or fetch.',
    features: ['OpenAPI spec', 'Full CRUD + search', 'Transaction support'],
    href: '/rest-api/introduction'
  },
  {
    icon: () => <Monitor size={24} strokeWidth={1.5} />,
    label: 'MCP Server',
    badge: 'MCP',
    badgeColor: '#7c3aed',
    description: 'Model Context Protocol server. Give any LLM agent direct database access.',
    features: ['Claude, GPT & Cursor', 'Schema auto-exposed', 'One-command deploy'],
    href: '/mcp-server/introduction'
  }
]

type ResourceCard = {
  icon: () => React.ReactElement
  label: string
  description: string
  cta: string
  href: string
}

const RESOURCE_CARDS: ResourceCard[] = [
  {
    icon: () => <Rocket size={24} strokeWidth={1.5} />,
    label: 'Quick Tutorial',
    description: 'First write → graph → semantic search in under 10 minutes.',
    cta: 'Start here',
    href: '/get-started/quick-tutorial'
  },
  {
    icon: () => <Book size={24} strokeWidth={1.5} />,
    label: 'Tutorials',
    description: 'Hands-on guides: RAG pipelines, deployment, vector search, agent memory.',
    cta: 'Browse tutorials',
    href: '/tutorials'
  },
  {
    icon: () => <Sun size={24} strokeWidth={1.5} />,
    label: 'Concepts',
    description: 'How RushDB stores, links, and queries data — records, properties, graphs.',
    cta: 'Learn concepts',
    href: '/concepts'
  }
]

type FeatureItem = {
  icon: () => React.ReactElement
  title: string
  description: string
  accent: string
  href: string
}

const FEATURE_ITEMS: FeatureItem[] = [
  {
    icon: () => <Upload size={20} strokeWidth={1.5} />,
    title: 'Ingest anything',
    description:
      'Push flat objects, nested trees, or batches. Types inferred, graph built — no schema needed.',
    accent: '#3f81ff',
    href: '/concepts/data-ingestion'
  },
  {
    icon: () => <Share2 size={20} strokeWidth={1.5} />,
    title: 'Auto-linked graph',
    description: 'Nested objects become linked records automatically. Traverse relationships in queries.',
    accent: '#8b5cf6',
    href: '/concepts/relationships'
  },
  {
    icon: () => <Sparkles size={20} strokeWidth={1.5} />,
    title: 'Semantic search',
    description: 'Index any text property and query by meaning. Combine with field filters.',
    accent: '#f59e0b',
    href: '/concepts/semantic-search'
  },
  {
    icon: () => <Shield size={20} strokeWidth={1.5} />,
    title: 'ACID transactions',
    description: 'Wrap any combination of writes and reads. Nothing persists if any step fails.',
    accent: '#10b981',
    href: '/concepts/transactions'
  }
]

// ── Sub-components ────────────────────────────────────────────────────────────

function InterfaceCard({ card: c }: { card: InterfaceCard }) {
  return (
    <a
      href={c.href}
      className={`group flex flex-col border ${BORDER_CLASS} bg-[var(--ifm-card-background-color)] p-6 text-inherit no-underline transition-[background-color,border-color] duration-150 ease-out hover:bg-[var(--ifm-color-emphasis-100)] hover:no-underline focus:no-underline`}
    >
      {/* Header */}
      <div className="mb-4 flex items-start justify-between">
        <div className="flex h-10 w-10 items-center justify-center bg-[var(--ifm-color-emphasis-100)]">
          <c.icon />
        </div>
        <span
          className="px-2 py-0.5 text-[11px] font-bold uppercase tracking-wide text-white"
          style={{ backgroundColor: c.badgeColor }}
        >
          {c.badge}
        </span>
      </div>

      {/* Title & description */}
      <h3 className="mb-2 text-[16px] font-bold leading-snug text-[var(--ifm-font-color-base)]">{c.label}</h3>
      <p className="grow pb-4 text-sm leading-relaxed text-[var(--ifm-color-emphasis-700)]">
        {c.description}
      </p>

      {/* Features */}
      <ul className={`mb-5 space-y-1.5 border-t ${BORDER_CLASS} pt-4`}>
        {c.features.map((f) => (
          <li key={f} className="flex items-center gap-2 text-[13px] text-[var(--ifm-color-emphasis-700)]">
            <span className="text-[var(--ifm-color-primary)]">
              <Check size={13} strokeWidth={2.5} />
            </span>
            {f}
          </li>
        ))}
      </ul>

      {/* CTA */}
      <span className="flex items-center gap-1.5 text-[13px] font-semibold text-[var(--ifm-color-emphasis-600)] transition-colors duration-150 ease-out group-hover:text-[var(--ifm-font-color-base)]">
        Get started <ArrowRight size={14} />
      </span>
    </a>
  )
}

function ResourceCard({ card: c }: { card: ResourceCard }) {
  return (
    <a
      href={c.href}
      className={`group flex flex-col border ${BORDER_CLASS} bg-[var(--ifm-card-background-color)] p-6 text-inherit no-underline transition-[background-color,border-color] duration-150 ease-out hover:bg-[var(--ifm-color-emphasis-100)] hover:no-underline focus:no-underline`}
    >
      <div className="mb-4 flex h-10 w-10 items-center justify-center bg-[var(--ifm-color-emphasis-100)] text-[var(--ifm-color-primary)]">
        <c.icon />
      </div>
      <h3 className="mb-2 text-[16px] font-bold leading-snug text-[var(--ifm-font-color-base)]">{c.label}</h3>
      <p className="grow pb-4 text-sm leading-relaxed text-[var(--ifm-color-emphasis-700)]">
        {c.description}
      </p>
      <span className="flex items-center gap-1.5 text-[13px] font-semibold text-[var(--ifm-color-emphasis-600)] transition-colors duration-150 ease-out group-hover:text-[var(--ifm-font-color-base)]">
        {c.cta} <ArrowRight size={14} />
      </span>
    </a>
  )
}

function FeaturePill({ item }: { item: FeatureItem }) {
  return (
    <a
      href={item.href}
      className={`group flex items-start gap-4 border ${BORDER_CLASS} bg-[var(--ifm-card-background-color)] p-5 text-inherit no-underline transition-[background-color] duration-150 ease-out hover:bg-[var(--ifm-color-emphasis-100)] hover:no-underline focus:no-underline`}
    >
      <span
        className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center"
        style={{ backgroundColor: item.accent + '18', color: item.accent }}
      >
        <item.icon />
      </span>
      <div>
        <p className="mb-1 text-[14px] font-bold text-[var(--ifm-font-color-base)]">{item.title}</p>
        <p className="m-0 text-[13px] leading-relaxed text-[var(--ifm-color-emphasis-600)]">
          {item.description}
        </p>
      </div>
    </a>
  )
}

// ── Main component ────────────────────────────────────────────────────────────

export default function DocsHomePage() {
  return (
    <div className="language-tabs not-prose">
      {/* ── Hero ─────────────────────────────────────────────────────────── */}
      <div className="mb-12 mt-10 text-center">
        <span className="mb-6 inline-block font-mono text-sm uppercase tracking-widest text-[var(--ifm-color-emphasis-500)]">
          [ instant graph + vector storage ]
        </span>

        <h1 className="mb-6">
          <span className="block font-mono text-[2.5rem] font-bold leading-tight tracking-tight text-[var(--ifm-font-color-base)]">
            Memory for agents
          </span>
          <span className="block font-mono text-[2.5rem] font-bold leading-tight tracking-tight text-[var(--ifm-font-color-base)]">
            and apps. <span style={{ color: 'var(--ifm-color-primary)' }}>Instant.</span>
          </span>
        </h1>

        <p className="mx-auto mb-10 max-w-[540px] text-[1.05rem] leading-relaxed text-[var(--ifm-color-emphasis-600)]">
          Push any JSON or CSV. Get graph relationships and vector search automatically.
          <br />
          No schema. No pipeline. No glue code.
        </p>

        <div className="flex flex-wrap items-center justify-center gap-3">
          <a
            href="/get-started/quick-tutorial"
            className="inline-flex items-center gap-2 bg-[var(--ifm-color-primary)] px-5 py-2.5 text-[14px] font-semibold text-[var(--ifm-background-color)] no-underline transition-opacity duration-150 hover:text-[var(--ifm-background-color)] hover:no-underline hover:opacity-90"
            style={{ borderRadius: 0, color: 'var(--ifm-background-color)' }}
          >
            Quick Tutorial <ArrowRight size={14} />
          </a>
          <a
            href="https://app.rushdb.com"
            target="_blank"
            rel="noopener noreferrer"
            className={`inline-flex items-center gap-2 border ${BORDER_CLASS} bg-[var(--ifm-card-background-color)] px-5 py-2.5 text-[14px] font-semibold text-[var(--ifm-font-color-base)] no-underline transition-[background-color] duration-150 hover:bg-[var(--ifm-color-emphasis-100)] hover:text-[var(--ifm-font-color-base)] hover:no-underline`}
            style={{ borderRadius: 0 }}
          >
            Try RushDB Cloud →
          </a>
        </div>
      </div>

      {/* ── Features grid ────────────────────────────────────────────────── */}
      <div className="mb-12 grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
        {FEATURE_ITEMS.map((item) => (
          <FeaturePill key={item.title} item={item} />
        ))}
      </div>

      {/* ── Interfaces ───────────────────────────────────────────────────── */}
      <div className="mb-12">
        <div className="mb-6">
          <h2 className="mb-1.5 text-[1.4rem] font-bold text-[var(--ifm-font-color-base)]">
            Choose your interface
          </h2>
          <p className="m-0 text-[0.95rem] text-[var(--ifm-color-emphasis-600)]">
            Every interface gives you the same capabilities — pick the one that fits your stack.
          </p>
        </div>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
          {INTERFACE_CARDS.map((card) => (
            <InterfaceCard key={card.href} card={card} />
          ))}
        </div>
      </div>

      {/* ── Divider ──────────────────────────────────────────────────────── */}
      <hr className={`mb-12 border-0 border-t border-solid ${BORDER_CLASS}`} />

      {/* ── Learning resources ───────────────────────────────────────────── */}
      <div className="mb-12">
        <div className="mb-6">
          <h2 className="mb-1.5 text-[1.4rem] font-bold text-[var(--ifm-font-color-base)]">Start learning</h2>
          <p className="m-0 text-[0.95rem] text-[var(--ifm-color-emphasis-600)]">
            Guides, tutorials, and concept explanations to get you productive fast.
          </p>
        </div>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
          {RESOURCE_CARDS.map((card) => (
            <ResourceCard key={card.href} card={card} />
          ))}
        </div>
      </div>

      {/* ── Deployment CTA ───────────────────────────────────────────────── */}
      <div className={`grid grid-cols-1 gap-4 sm:grid-cols-2`}>
        <div className={`border ${BORDER_CLASS} bg-[var(--ifm-card-background-color)] p-6`}>
          <p className="mb-1 text-[13px] font-bold uppercase tracking-widest text-[var(--ifm-color-emphasis-500)]">
            Cloud
          </p>
          <h3 className="mb-2 text-[17px] font-bold text-[var(--ifm-font-color-base)]">RushDB Cloud</h3>
          <p className="mb-4 text-sm leading-relaxed text-[var(--ifm-color-emphasis-600)]">
            Free tier — 100,000 KU/month and 2 projects. No credit card required.
          </p>
          <a
            href="https://app.rushdb.com"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-1.5 text-[13px] font-semibold text-[var(--ifm-color-primary)] no-underline hover:no-underline hover:opacity-80"
          >
            Sign up free <ArrowRight size={14} />
          </a>
        </div>

        <div className={`border ${BORDER_CLASS} bg-[var(--ifm-card-background-color)] p-6`}>
          <p className="mb-1 text-[13px] font-bold uppercase tracking-widest text-[var(--ifm-color-emphasis-500)]">
            Self-Hosted
          </p>
          <h3 className="mb-2 text-[17px] font-bold text-[var(--ifm-font-color-base)]">
            Run on your infrastructure
          </h3>
          <p className="mb-4 text-sm leading-relaxed text-[var(--ifm-color-emphasis-600)]">
            No KU limits, no billing. Deploy with Docker in minutes on your own stack.
          </p>
          <a
            href="/tutorials/deployment"
            className="inline-flex items-center gap-1.5 text-[13px] font-semibold text-[var(--ifm-color-primary)] no-underline hover:no-underline hover:opacity-80"
          >
            Deployment guide <ArrowRight size={14} />
          </a>
        </div>
      </div>
    </div>
  )
}
