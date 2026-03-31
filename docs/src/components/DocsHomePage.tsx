import React from 'react'

const BORDER_CLASS = 'border-[var(--ifm-color-emphasis-200)]'

// ── Icons ─────────────────────────────────────────────────────────────────────

const ArrowIcon = () => (
  <svg
    width="14"
    height="14"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <path d="M5 12h14" />
    <path d="m12 5 7 7-7 7" />
  </svg>
)

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

const RestIcon = () => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    viewBox="0 0 24 24"
    width="24"
    height="24"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.5"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <path d="M12 2a10 10 0 1 0 0 20A10 10 0 0 0 12 2z" />
    <path d="M2 12h20" />
    <path d="M12 2c-2.76 3.45-4 7-4 10s1.24 6.55 4 10" />
    <path d="M12 2c2.76 3.45 4 7 4 10s-1.24 6.55-4 10" />
  </svg>
)

const McpIcon = () => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    viewBox="0 0 24 24"
    width="24"
    height="24"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.5"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <rect x="2" y="3" width="20" height="14" rx="2" />
    <path d="M8 21h8" />
    <path d="M12 17v4" />
    <path d="M7 7h.01" />
    <path d="M11 7h6" />
    <path d="M7 11h.01" />
    <path d="M11 11h6" />
  </svg>
)

const BookIcon = () => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    viewBox="0 0 24 24"
    width="24"
    height="24"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.5"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
    <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
  </svg>
)

const RocketIcon = () => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    viewBox="0 0 24 24"
    width="24"
    height="24"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.5"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <path d="M4.5 16.5c-1.5 1.26-2 5-2 5s3.74-.5 5-2c.71-.84.7-2.13-.09-2.91a2.18 2.18 0 0 0-2.91-.09z" />
    <path d="m12 15-3-3a22 22 0 0 1 2-3.95A12.88 12.88 0 0 1 22 2c0 2.72-.78 7.5-6 11a22.35 22.35 0 0 1-4 2z" />
    <path d="M9 12H4s.55-3.03 2-4c1.62-1.08 5 0 5 0" />
    <path d="M12 15v5s3.03-.55 4-2c1.08-1.62 0-5 0-5" />
  </svg>
)

const ConceptsIcon = () => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    viewBox="0 0 24 24"
    width="24"
    height="24"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.5"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <circle cx="12" cy="12" r="3" />
    <path d="M12 2v3" />
    <path d="M12 19v3" />
    <path d="M4.22 4.22l2.12 2.12" />
    <path d="M17.66 17.66l2.12 2.12" />
    <path d="M2 12h3" />
    <path d="M19 12h3" />
    <path d="M4.22 19.78l2.12-2.12" />
    <path d="M17.66 6.34l2.12-2.12" />
  </svg>
)

const IngestIcon = () => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    viewBox="0 0 24 24"
    width="20"
    height="20"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.5"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
    <polyline points="17 8 12 3 7 8" />
    <line x1="12" y1="3" x2="12" y2="15" />
  </svg>
)

const FilterIcon = () => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    viewBox="0 0 24 24"
    width="20"
    height="20"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.5"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3" />
  </svg>
)

const SparkleIcon = () => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    viewBox="0 0 24 24"
    width="20"
    height="20"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.5"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z" />
    <path d="M19 3v4" />
    <path d="M21 5h-4" />
  </svg>
)

const ShieldIcon = () => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    viewBox="0 0 24 24"
    width="20"
    height="20"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.5"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
  </svg>
)

const GraphIcon = () => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    viewBox="0 0 24 24"
    width="20"
    height="20"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.5"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <circle cx="18" cy="5" r="3" />
    <circle cx="6" cy="12" r="3" />
    <circle cx="18" cy="19" r="3" />
    <line x1="8.59" y1="13.51" x2="15.42" y2="17.49" />
    <line x1="15.41" y1="6.51" x2="8.59" y2="10.49" />
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
    icon: RestIcon,
    label: 'REST API',
    badge: 'HTTP',
    badgeColor: '#16a34a',
    description: 'Language-agnostic HTTP access. Works from any stack with curl or fetch.',
    features: ['OpenAPI spec', 'Full CRUD + search', 'Transaction support'],
    href: '/rest-api/introduction'
  },
  {
    icon: McpIcon,
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
    icon: RocketIcon,
    label: 'Quick Tutorial',
    description: 'First write → graph → semantic search in under 10 minutes.',
    cta: 'Start here',
    href: '/get-started/quick-tutorial'
  },
  {
    icon: BookIcon,
    label: 'Tutorials',
    description: 'Hands-on guides: RAG pipelines, deployment, vector search, agent memory.',
    cta: 'Browse tutorials',
    href: '/tutorials'
  },
  {
    icon: ConceptsIcon,
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
}

const FEATURE_ITEMS: FeatureItem[] = [
  {
    icon: IngestIcon,
    title: 'Ingest anything',
    description:
      'Push flat objects, nested trees, or batches. Types inferred, graph built — no schema needed.',
    accent: '#3f81ff'
  },
  {
    icon: GraphIcon,
    title: 'Auto-linked graph',
    description: 'Nested objects become linked records automatically. Traverse relationships in queries.',
    accent: '#8b5cf6'
  },
  {
    icon: SparkleIcon,
    title: 'Semantic search',
    description: 'Index any text property and query by meaning. Combine with field filters.',
    accent: '#f59e0b'
  },
  {
    icon: ShieldIcon,
    title: 'ACID transactions',
    description: 'Wrap any combination of writes and reads. Nothing persists if any step fails.',
    accent: '#10b981'
  }
]

// ── Sub-components ────────────────────────────────────────────────────────────

function CheckIcon() {
  return (
    <svg
      width="13"
      height="13"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <polyline points="20 6 9 17 4 12" />
    </svg>
  )
}

function InterfaceCard({ card: c }: { card: InterfaceCard }) {
  return (
    <a
      href={c.href}
      className={`group flex flex-col rounded-xl border ${BORDER_CLASS} bg-[var(--ifm-card-background-color)] p-6 text-inherit no-underline transition-[background-color,border-color] duration-150 ease-out hover:bg-[var(--ifm-color-emphasis-100)] hover:no-underline focus:no-underline`}
    >
      {/* Header */}
      <div className="mb-4 flex items-start justify-between">
        <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-[var(--ifm-color-emphasis-100)]">
          <c.icon />
        </div>
        <span
          className="rounded-full px-2 py-0.5 text-[11px] font-bold uppercase tracking-wide text-white"
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
              <CheckIcon />
            </span>
            {f}
          </li>
        ))}
      </ul>

      {/* CTA */}
      <span className="flex items-center gap-1.5 text-[13px] font-semibold text-[var(--ifm-color-emphasis-600)] transition-colors duration-150 ease-out group-hover:text-[var(--ifm-font-color-base)]">
        Get started <ArrowIcon />
      </span>
    </a>
  )
}

function ResourceCard({ card: c }: { card: ResourceCard }) {
  return (
    <a
      href={c.href}
      className={`group flex flex-col rounded-xl border ${BORDER_CLASS} bg-[var(--ifm-card-background-color)] p-6 text-inherit no-underline transition-[background-color,border-color] duration-150 ease-out hover:bg-[var(--ifm-color-emphasis-100)] hover:no-underline focus:no-underline`}
    >
      <div className="mb-4 flex h-10 w-10 items-center justify-center rounded-lg bg-[var(--ifm-color-emphasis-100)] text-[var(--ifm-color-primary)]">
        <c.icon />
      </div>
      <h3 className="mb-2 text-[16px] font-bold leading-snug text-[var(--ifm-font-color-base)]">{c.label}</h3>
      <p className="grow pb-4 text-sm leading-relaxed text-[var(--ifm-color-emphasis-700)]">
        {c.description}
      </p>
      <span className="flex items-center gap-1.5 text-[13px] font-semibold text-[var(--ifm-color-emphasis-600)] transition-colors duration-150 ease-out group-hover:text-[var(--ifm-font-color-base)]">
        {c.cta} <ArrowIcon />
      </span>
    </a>
  )
}

function FeaturePill({ item }: { item: FeatureItem }) {
  return (
    <div
      className={`flex items-start gap-4 rounded-xl border ${BORDER_CLASS} bg-[var(--ifm-card-background-color)] p-5`}
    >
      <span
        className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg"
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
    </div>
  )
}

// ── Main component ────────────────────────────────────────────────────────────

export default function DocsHomePage() {
  return (
    <div className="language-tabs not-prose">
      {/* ── Hero ─────────────────────────────────────────────────────────── */}
      <div className="mb-12 mt-10 text-center">
        <div className="mb-4 inline-flex items-center gap-2 rounded-full border border-[var(--ifm-color-emphasis-200)] bg-[var(--ifm-card-background-color)] px-3.5 py-1.5 text-[13px] font-medium text-[var(--ifm-color-emphasis-600)]">
          <span className="h-1.5 w-1.5 rounded-full bg-[#10b981]" />
          Make AI Agents and Apps Conciuous
        </div>

        <h1 className="mb-4 text-[2.6rem] font-bold leading-[1.2] tracking-tight text-[var(--ifm-font-color-base)]">
          Welcome to <span style={{ color: 'var(--ifm-color-primary)' }}>RushDB</span>
        </h1>

        <p className="mx-auto mb-8 max-w-[560px] text-[1.1rem] leading-relaxed text-[var(--ifm-color-emphasis-600)]">
          Push any JSON — RushDB infers types, links nested objects into a graph, and makes everything
          queryable by field value or by meaning. No schema design. No migrations. No separate vector store.
        </p>

        <div className="flex flex-wrap items-center justify-center gap-3">
          <a
            href="/get-started/quick-tutorial"
            className="inline-flex items-center gap-2 rounded-lg bg-[var(--ifm-color-primary)] px-5 py-2.5 text-[14px] font-semibold text-[var(--ifm-background-color)] no-underline transition-opacity duration-150 hover:text-[var(--ifm-background-color)] hover:no-underline hover:opacity-90"
          >
            Quick Tutorial <ArrowIcon />
          </a>
          <a
            href="https://app.rushdb.com"
            target="_blank"
            rel="noopener noreferrer"
            className={`inline-flex items-center gap-2 rounded-lg border ${BORDER_CLASS} bg-[var(--ifm-card-background-color)] px-5 py-2.5 text-[14px] font-semibold text-[var(--ifm-font-color-base)] no-underline transition-[background-color] duration-150 hover:bg-[var(--ifm-color-emphasis-100)] hover:text-[var(--ifm-font-color-base)] hover:no-underline`}
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
        <div className={`rounded-xl border ${BORDER_CLASS} bg-[var(--ifm-card-background-color)] p-6`}>
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
            Sign up free <ArrowIcon />
          </a>
        </div>

        <div className={`rounded-xl border ${BORDER_CLASS} bg-[var(--ifm-card-background-color)] p-6`}>
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
            Deployment guide <ArrowIcon />
          </a>
        </div>
      </div>
    </div>
  )
}
