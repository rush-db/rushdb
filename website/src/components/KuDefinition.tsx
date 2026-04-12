import type { ReactNode } from 'react'
import type { LucideIcon } from 'lucide-react'
import { Database, GitBranch, Search, Zap } from 'lucide-react'
import { motion } from 'framer-motion'
import { fadeUp } from '~/components/lp/ui'

/** 'lp' — LP landing-page design (CSS vars, font-mono, framer-motion)
 *  'site' — Main site design system (Tailwind classes) */
export type KuDefinitionVariant = 'lp' | 'site'

interface KuDefinitionProps {
  variant?: KuDefinitionVariant
}

export function KuDefinition({ variant = 'site' }: KuDefinitionProps) {
  const isLp = variant === 'lp'

  const iconCls = isLp ? 'h-4 w-4' : 'text-accent h-4 w-4 shrink-0'
  const iconStyle = isLp ? { color: 'var(--lp-accent)' } : undefined

  const hl = (text: string): JSX.Element =>
    isLp ?
      <span className="text-lp-text font-bold">{text}</span>
    : <strong className="text-content2">{text}</strong>

  const ac = (text: string): JSX.Element =>
    isLp ?
      <span style={{ color: 'var(--lp-accent)' }} className="font-bold">
        {text}
      </span>
    : <strong className="text-accent">{text}</strong>

  const cards: Array<{ Icon: LucideIcon; label: string; text: ReactNode }> = [
    {
      Icon: Database,
      label: 'Flat records',
      text: (
        <>
          A record with {hl('10 properties')} costs {ac('~10 KU')} per write (0.5 base + 1 per property).
        </>
      )
    },
    {
      Icon: GitBranch,
      label: 'Nested objects',
      text: (
        <>
          Nested objects are {hl('decomposed into separate linked records')}. Each child contributes its own
          KU; each link costs 0.25 KU.
        </>
      )
    },
    {
      Icon: Search,
      label: 'Reads always free',
      text: (
        <>
          Standard queries {hl('never consume KU')}. Embeddings cost 5 KU per record; vector search, raw
          Cypher, and deep traversals cost 5 KU per call.
        </>
      )
    }
  ]

  const body = (
    <>
      <div className="mb-6 flex flex-wrap items-start justify-between gap-4">
        <div>
          <div className="mb-2 flex items-center gap-2">
            <Zap className={isLp ? 'h-4 w-4' : 'text-accent h-5 w-5'} style={iconStyle} />
            {isLp ?
              <span className="text-lp-text font-mono text-sm uppercase tracking-widest">
                What is a Knowledge Unit (KU)?
              </span>
            : <h4 className="text-content text-lg font-bold">What is a Knowledge Unit (KU)?</h4>}
          </div>
          <p
            className={
              isLp ?
                'text-lp-muted max-w-2xl font-mono text-sm leading-relaxed'
              : 'text-content3 max-w-2xl text-sm font-medium'
            }
          >
            A {hl('Knowledge Unit')} is RushDB&apos;s atomic measure of structured knowledge created during a
            write operation. KU accumulates from records created, properties stored, relationships formed, and
            embeddings generated. Standard reads and queries are always free.
          </p>
        </div>
        <span
          className={
            isLp ?
              'font-mono text-sm uppercase tracking-wide'
            : 'text-accent border-accent/40 shrink-0 rounded-full border px-3 py-1 text-xs font-bold uppercase tracking-wide'
          }
          style={
            isLp ?
              {
                color: 'var(--lp-accent)',
                border: '1px solid rgba(var(--lp-accent-rgb), 0.3)',
                padding: '4px 12px'
              }
            : undefined
          }
        >
          0.5 / record · 1 / property · 0.25 / link
        </span>
      </div>

      <div className={`grid grid-cols-3 gap-4 sm:grid-cols-1${isLp ? '' : 'md:grid-cols-3'}`}>
        {cards.map(({ Icon, label, text }) => (
          <div
            key={label}
            className={isLp ? 'border p-4' : 'bg-fill2 rounded-xl p-4'}
            style={isLp ? { borderColor: 'var(--lp-border)' } : undefined}
          >
            <div className="mb-2 flex items-center gap-2">
              <Icon className={iconCls} style={iconStyle} />
              <span
                className={
                  isLp ?
                    'text-lp-text font-mono text-sm uppercase tracking-wider'
                  : 'text-content text-sm font-semibold'
                }
              >
                {label}
              </span>
            </div>
            <p
              className={
                isLp ?
                  'text-lp-muted font-mono text-sm leading-relaxed'
                : 'text-content3 text-xs leading-relaxed'
              }
            >
              {text}
            </p>
          </div>
        ))}
      </div>
    </>
  )

  if (isLp) {
    return (
      <motion.div
        {...fadeUp(0.1)}
        className="mb-16 border p-8"
        style={{ borderColor: 'var(--lp-border)', background: 'rgba(var(--lp-accent-rgb), 0.03)' }}
      >
        {body}
      </motion.div>
    )
  }

  return <div className="border-accent/30 bg-accent/5 w-full rounded-2xl border p-6 md:p-8">{body}</div>
}
