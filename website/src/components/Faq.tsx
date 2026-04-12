import type { ReactNode } from 'react'
import { useState } from 'react'
import { ChevronDown } from 'lucide-react'
import cx from 'classnames'

/** 'lp'   — LP landing-page design (CSS vars, font-mono)
 *  'site' — Main site design system (Tailwind classes) */
export type FaqItemVariant = 'lp' | 'site'

export function FaqItem({
  question,
  answer,
  variant = 'lp'
}: {
  question: string
  answer: ReactNode
  variant?: FaqItemVariant
}) {
  const [open, setOpen] = useState(false)
  const isLp = variant === 'lp'

  return (
    <div
      className={cx(!isLp && 'border-stroke border-b last:border-b-0')}
      style={isLp ? { borderBottom: '1px solid var(--lp-border)' } : undefined}
    >
      <button
        onClick={() => setOpen(!open)}
        className={cx(
          'flex w-full items-center justify-between py-5 text-left transition-colors',
          isLp ? 'text-lp-text font-mono text-sm' : 'text-content hover:text-accent font-medium'
        )}
        style={isLp ? { background: 'none', border: 'none', cursor: 'pointer' } : undefined}
      >
        <span className={cx(!isLp && 'text-base font-semibold')}>{question}</span>
        <ChevronDown
          className={cx(
            'shrink-0 transition-transform',
            open && 'rotate-180',
            isLp ? '' : 'text-accent h-5 w-5'
          )}
          style={
            isLp ?
              {
                color: 'var(--lp-accent)',
                transform: open ? 'rotate(180deg)' : 'none',
                width: 16,
                height: 16
              }
            : undefined
          }
        />
      </button>
      {open && (
        <div
          className={cx(
            'pb-5',
            isLp ? 'text-lp-muted font-mono text-sm leading-relaxed' : 'text-content3 text-sm leading-relaxed'
          )}
        >
          {answer}
        </div>
      )}
    </div>
  )
}

export const Faq = ({
  items = [
    {
      question: 'What exactly happens when I push nested JSON?',
      answer:
        'RushDB decomposes your JSON depth-first. Each nested object becomes a separate typed record. The parent key becomes the label. Parent-child relationships are created automatically. Scalar properties stay on their owning record, arrays of objects become multiple child records, and types are inferred (string, number, boolean, datetime). One importJson call with a 4-level-deep object might create 10+ records with all relationships wired — no schema needed.'
    },
    {
      question: 'What about type safety? Can I enforce a schema when I want one?',
      answer:
        'Yes. RushDB offers optional Model classes with field-level validation: type, required, unique, multiple, and default. Define a Model when you need guarantees (e.g. "email must be a unique string"). Skip it when you want schemaless flexibility. Both modes use the same query DSL and the same records.find() API.'
    },
    {
      question: 'How does vector search work alongside graph queries?',
      answer:
        'Create an embedding index on any string property — managed (auto-embed with built-in model) or external (bring your own vectors). Then query with ai.search() for semantic similarity, or add vector.similarity operators inside a regular records.find() aggregate. The key part: vector results are filtered by the same structural where clause. So "find products similar to this description WHERE category = electronics AND price < 1000" is one query, not two systems glued together.'
    },
    {
      question: 'How does RushDB compare to Postgres + Prisma?',
      answer:
        'With Postgres + Prisma, every new entity needs a schema definition, a migration, and ORM model updates. Cross-entity queries require explicit JOINs or nested includes. Schema changes require migration files. With RushDB, you push JSON — done. Cross-entity queries use the same JSON DSL with $alias for traversal. Schema changes? Just push JSON with new fields. The tradeoff: RushDB is optimized for graph-shaped data and flexible queries, not row-level SQL analytics.'
    },
    {
      question: 'How does billing work — what is a Knowledge Unit?',
      answer:
        'A Knowledge Unit (KU) is the atomic measure of structured data in RushDB. A record with 10 properties = 10 KU. Nested objects decompose into linked records, each contributing its own KU. Standard reads and queries are always free. The free plan includes 100K KU/month — enough for thousands of records. Paid plans start at $29/month.'
    },
    {
      question: 'Can I self-host RushDB?',
      answer:
        'Yes. Docker container + your own Neo4j instance — no limits, no billing, full control. One command: docker run -p 3000:3000 rushdb/platform with your Neo4j credentials. RushDB Cloud is also available with a free tier (100K KU/month, 2 projects, no credit card) if you prefer not to manage infrastructure.'
    }
  ],
  className
}: {
  items?: { question: string; answer: string }[]
  className?: string
}) => {
  return (
    <div className={className}>
      <p className="text-lp-muted mb-4 mt-12 font-mono text-sm uppercase tracking-widest">FAQ</p>
      <div style={{ borderTop: '1px solid var(--lp-border)' }}>
        {items.map((item, index) => (
          <FaqItem key={index} {...item} />
        ))}
      </div>
    </div>
  )
}
