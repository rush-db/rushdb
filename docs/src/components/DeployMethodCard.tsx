import React from 'react'
import Link from '@docusaurus/Link'
import {
  ArrowRight,
  Box,
  Check,
  ClipboardList,
  Clock,
  Cloud,
  Code2,
  ExternalLink,
  Globe,
  Layers,
  Server,
  Terminal,
  type LucideIcon
} from 'lucide-react'
import { cn } from './ui/lib/utils'

const ICON_MAP: Record<string, LucideIcon> = {
  Box,
  ClipboardList,
  Cloud,
  Code2,
  Globe,
  Layers,
  Server
}

const BORDER_CLASS = 'border-[var(--ifm-color-emphasis-200)]'

// ── Badge config ───────────────────────────────────────────────────────────────

type BadgeVariant = 'beginner' | 'intermediate' | 'advanced' | 'cloud' | 'self-hosted' | 'byoc' | 'required'

const BADGE_STYLES: Record<BadgeVariant, { label: string; bg: string; color: string }> = {
  beginner: { label: 'Beginner', bg: '#16a34a', color: '#fff' },
  intermediate: { label: 'Intermediate', bg: '#d97706', color: '#fff' },
  advanced: { label: 'Advanced', bg: '#dc2626', color: '#fff' },
  cloud: { label: 'Cloud', bg: '#3f81ff', color: '#fff' },
  'self-hosted': { label: 'Self-Hosted', bg: '#7c3aed', color: '#fff' },
  byoc: { label: 'BYOC', bg: '#0891b2', color: '#fff' },
  required: { label: 'Required', bg: '#475569', color: '#fff' }
}

// ── Types ──────────────────────────────────────────────────────────────────────

export type DeployMethodCardProps = {
  title: string
  description: string
  badge: BadgeVariant
  time?: string
  prerequisites?: string[]
  commands?: string[]
  tags?: string[]
  href: string
  recommended?: boolean
  icon?: string
}

// ── Card ───────────────────────────────────────────────────────────────────────

export function DeployMethodCard({
  title,
  description,
  badge,
  time,
  prerequisites = [],
  commands = [],
  tags = [],
  href
}: DeployMethodCardProps) {
  const b = BADGE_STYLES[badge]

  return (
    <Link
      to={href}
      className={cn(
        'group flex flex-col border',
        BORDER_CLASS,
        'bg-[var(--ifm-card-background-color)] p-6 text-inherit no-underline',
        'transition-[background-color,border-color] duration-150 ease-out',
        'hover:bg-[var(--ifm-color-emphasis-100)] hover:no-underline focus:no-underline'
      )}
    >
      {/* Header row */}
      <div className="mb-4 flex items-start justify-between gap-3">
        <h3 className="m-0 text-[16px] font-bold leading-snug text-[var(--ifm-font-color-base)]">{title}</h3>
        <div className="flex shrink-0 items-center gap-2">
          {time && (
            <span className="flex items-center gap-1 text-[11px] text-[var(--ifm-color-emphasis-500)]">
              <Clock size={11} strokeWidth={2} />
              {time}
            </span>
          )}
          <span
            className="px-2 py-0.5 text-[11px] font-bold uppercase tracking-wide"
            style={{ backgroundColor: b.bg, color: b.color }}
          >
            {b.label}
          </span>
        </div>
      </div>

      {/* Description */}
      <p className="mb-4 text-sm leading-relaxed text-[var(--ifm-color-emphasis-700)]">{description}</p>

      {/* Prerequisites + Commands */}
      {(prerequisites.length > 0 || commands.length > 0) && (
        <div
          className={cn(
            'mb-4 grid gap-4 border-t pt-4',
            BORDER_CLASS,
            commands.length > 0 && prerequisites.length > 0 ? 'grid-cols-2' : 'grid-cols-1'
          )}
        >
          {prerequisites.length > 0 && (
            <div>
              <p className="mb-2 text-[11px] font-bold uppercase tracking-wide text-[var(--ifm-color-emphasis-500)]">
                Prerequisites
              </p>
              <ul className="m-0 list-none space-y-1.5 p-0">
                {prerequisites.map((p) => (
                  <li
                    key={p}
                    className="flex items-center gap-1.5 text-[13px] text-[var(--ifm-color-emphasis-700)]"
                  >
                    <Check size={12} strokeWidth={2.5} className="shrink-0 text-[var(--ifm-color-primary)]" />
                    {p}
                  </li>
                ))}
              </ul>
            </div>
          )}
          {commands.length > 0 && (
            <div>
              <p className="mb-2 text-[11px] font-bold uppercase tracking-wide text-[var(--ifm-color-emphasis-500)]">
                Key Commands
              </p>
              <ul className="m-0 list-none space-y-1.5 p-0">
                {commands.map((cmd) => (
                  <li
                    key={cmd}
                    className="flex items-start gap-1.5 font-mono text-[12px] text-[var(--ifm-color-emphasis-700)]"
                  >
                    <Terminal
                      size={12}
                      strokeWidth={2}
                      className="mt-0.5 shrink-0 text-[var(--ifm-color-emphasis-500)]"
                    />
                    <code className="m-0 truncate bg-transparent p-0 text-[12px]">{cmd}</code>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}

      {/* Tags */}
      {tags.length > 0 && (
        <div className="mb-4 flex flex-wrap gap-1.5">
          {tags.map((t) => (
            <span
              key={t}
              className="bg-[var(--ifm-color-emphasis-100)] px-2 py-0.5 text-[11px] text-[var(--ifm-color-emphasis-600)]"
            >
              {t}
            </span>
          ))}
        </div>
      )}

      {/* CTA */}
      <span className="mt-auto flex items-center gap-1.5 text-[13px] font-semibold text-[var(--ifm-color-emphasis-600)] transition-colors duration-150 ease-out group-hover:text-[var(--ifm-font-color-base)]">
        View full guide <ArrowRight size={14} />
      </span>
    </Link>
  )
}

// ── Grid wrapper ───────────────────────────────────────────────────────────────

export function DeployMethodCardGrid({ children }: { children: React.ReactNode }) {
  return <div className="not-prose grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">{children}</div>
}

// ── Cross-link section ─────────────────────────────────────────────────────────

type CrossLink = { label: string; href: string }
type DeployButtonVariant = 'primary' | 'secondary'
type DeployButton = {
  label: string
  href: string
  variant?: DeployButtonVariant
}

export function DeployCrossLinks({ prefix, links }: { prefix?: string; links: CrossLink[] }) {
  return (
    <div className={cn('not-prose mt-8 border-t pt-6', BORDER_CLASS)}>
      {prefix && <p className="mb-3 text-sm text-[var(--ifm-color-emphasis-600)]">{prefix}</p>}
      <div className="flex flex-wrap gap-3">
        {links.map((l) => (
          <Link
            key={l.href}
            to={l.href}
            className={cn(
              'flex items-center gap-1.5 border',
              BORDER_CLASS,
              'bg-[var(--ifm-card-background-color)] px-4 py-2 text-[13px] font-semibold text-inherit no-underline',
              'transition-[background-color] duration-150 ease-out hover:bg-[var(--ifm-color-emphasis-100)] hover:no-underline'
            )}
          >
            {l.label} <ArrowRight size={13} />
          </Link>
        ))}
      </div>
    </div>
  )
}

export function DeployButtonGroup({ buttons }: { buttons: DeployButton[] }) {
  return (
    <div className="not-prose my-6 flex flex-wrap gap-3">
      {buttons.map((button) => {
        const variant = button.variant ?? 'secondary'

        return (
          <Link
            key={button.href}
            to={button.href}
            className={cn(
              'inline-flex items-center gap-2 border px-4 py-2 text-sm font-semibold no-underline',
              'transition-[background-color,border-color,color] duration-150 ease-out hover:no-underline',
              variant === 'primary' ?
                'border-[var(--ifm-color-primary)] bg-[var(--ifm-color-primary)] text-[var(--rushdb-on-primary)] hover:text-[var(--rushdb-on-primary)]'
              : cn(
                  BORDER_CLASS,
                  'bg-[var(--ifm-card-background-color)] text-inherit hover:bg-[var(--ifm-color-emphasis-100)]'
                )
            )}
          >
            {button.label}
            <ExternalLink size={14} strokeWidth={2} />
          </Link>
        )
      })}
    </div>
  )
}

// ── Row (full-width) ───────────────────────────────────────────────────────────

export type DeployMethodRowProps = DeployMethodCardProps

export function DeployMethodRow({
  title,
  description,
  badge,
  time,
  prerequisites = [],
  commands = [],
  tags = [],
  href,
  recommended,
  icon
}: DeployMethodRowProps) {
  const b = BADGE_STYLES[badge]
  const Icon = icon ? ICON_MAP[icon] : null

  return (
    <Link
      to={href}
      className={cn(
        'group flex flex-col border',
        BORDER_CLASS,
        'bg-[var(--ifm-card-background-color)] p-6 text-inherit no-underline',
        'transition-[background-color,border-color] duration-150 ease-out',
        'hover:bg-[var(--ifm-color-emphasis-100)] hover:no-underline focus:no-underline'
      )}
    >
      {/* Header: icon + title/badges/description + time */}
      <div className="mb-4 flex items-start justify-between gap-4">
        <div className="flex items-start gap-3">
          {Icon && (
            <div
              className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full"
              style={{ backgroundColor: b.bg }}
            >
              <Icon size={18} color="#fff" strokeWidth={2} />
            </div>
          )}
          <div>
            <div className="mb-1 flex flex-wrap items-center gap-2">
              <h3 className="m-0 text-[16px] font-bold leading-snug text-[var(--ifm-font-color-base)]">
                {title}
              </h3>
              {recommended && (
                <span
                  className="px-2 py-0.5 text-[11px] font-bold uppercase tracking-wide"
                  style={{ backgroundColor: '#16a34a', color: '#fff' }}
                >
                  Recommended
                </span>
              )}
              <span
                className="px-2 py-0.5 text-[11px] font-bold uppercase tracking-wide"
                style={{ backgroundColor: b.bg, color: b.color }}
              >
                {b.label}
              </span>
            </div>
            <p className="m-0 text-sm leading-relaxed text-[var(--ifm-color-emphasis-700)]">{description}</p>
          </div>
        </div>
        {time && (
          <span className="flex shrink-0 items-center gap-1 text-[11px] text-[var(--ifm-color-emphasis-500)]">
            <Clock size={11} strokeWidth={2} />
            {time}
          </span>
        )}
      </div>

      {/* Prerequisites + Commands */}
      {(prerequisites.length > 0 || commands.length > 0) && (
        <div
          className={cn(
            'mb-4 grid gap-6 border-t pt-4',
            BORDER_CLASS,
            commands.length > 0 && prerequisites.length > 0 ? 'grid-cols-2' : 'grid-cols-1'
          )}
        >
          {prerequisites.length > 0 && (
            <div>
              <p className="mb-2 text-[11px] font-bold uppercase tracking-wide text-[var(--ifm-color-emphasis-500)]">
                Prerequisites
              </p>
              <ul className="m-0 list-none space-y-1.5 p-0">
                {prerequisites.map((p) => (
                  <li
                    key={p}
                    className="flex items-center gap-1.5 text-[13px] text-[var(--ifm-color-emphasis-700)]"
                  >
                    <Check size={12} strokeWidth={2.5} className="shrink-0 text-[var(--ifm-color-primary)]" />
                    {p}
                  </li>
                ))}
              </ul>
            </div>
          )}
          {commands.length > 0 && (
            <div>
              <p className="mb-2 text-[11px] font-bold uppercase tracking-wide text-[var(--ifm-color-emphasis-500)]">
                Key Commands
              </p>
              <ul className="m-0 list-none space-y-1.5 p-0">
                {commands.map((cmd) => (
                  <li
                    key={cmd}
                    className="flex items-start gap-1.5 font-mono text-[12px] text-[var(--ifm-color-emphasis-700)]"
                  >
                    <Terminal
                      size={12}
                      strokeWidth={2}
                      className="mt-0.5 shrink-0 text-[var(--ifm-color-emphasis-500)]"
                    />
                    <code className="m-0 bg-transparent p-0 text-[12px]">{cmd}</code>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}

      {/* Footer: tags + CTA */}
      <div className={cn('flex items-center justify-between gap-4 border-t pt-4', BORDER_CLASS)}>
        {tags.length > 0 ?
          <div className="flex flex-wrap gap-1.5">
            {tags.map((t) => (
              <span
                key={t}
                className="bg-[var(--ifm-color-emphasis-100)] px-2 py-0.5 text-[11px] text-[var(--ifm-color-emphasis-600)]"
              >
                {t}
              </span>
            ))}
          </div>
        : <div />}
        <span className="flex shrink-0 items-center gap-1.5 text-[13px] font-semibold text-[var(--ifm-color-emphasis-600)] transition-colors duration-150 ease-out group-hover:text-[var(--ifm-font-color-base)]">
          View full guide <ArrowRight size={14} />
        </span>
      </div>
    </Link>
  )
}

// ── Row list wrapper ───────────────────────────────────────────────────────────

export function DeployMethodRowList({ children }: { children: React.ReactNode }) {
  return <div className="not-prose flex flex-col gap-4">{children}</div>
}
