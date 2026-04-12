/**
 * LP Design System — shared primitives for all landing page sections.
 *
 * Tokens
 * ──────
 * bg-lp-bg      #0A0A0B   primary dark background
 * bg-lp-surface #111113   elevated surface (every other section)
 * border-lp-border / #1E1E22   dividers, card borders
 * text-lp-text  #F0F0EE   primary text
 * text-lp-muted #6B6B72   secondary / supporting text
 * text-lp-accent #00FF85  accent / highlight
 *
 * Type scale (mobile responsive)
 * ───────────────────────────────
 * text-4xl sm:text-xl  → section headings / hero H1+H2
 * text-base            → prominent sub-text
 * text-sm              → body copy, labels, buttons, card content
 */

import Link from 'next/link'
import { motion } from 'framer-motion'
import * as SwitchPrimitives from '@radix-ui/react-switch'
import type { ReactNode, HTMLAttributes } from 'react'

// ── Shared animation helper ───────────────────────────────────────────────
export const fadeUp = (delay = 0) => ({
  initial: { opacity: 0, y: 20 },
  whileInView: { opacity: 1, y: 0 },
  viewport: { once: true },
  transition: { duration: 0.45, delay, ease: 'easeOut' }
})

// ── Section wrapper ───────────────────────────────────────────────────────
interface LPSectionProps extends HTMLAttributes<HTMLElement> {
  /** Use the elevated surface colour (#111113) instead of bg-lp-bg (#0A0A0B) */
  surface?: boolean
  /** Draw a 1px border on top */
  borderTop?: boolean
  /** Draw 1px borders on top AND bottom */
  borderY?: boolean
  children: ReactNode
}

export function LPSection({
  surface = false,
  borderTop = false,
  borderY = false,
  className = '',
  children,
  ...rest
}: LPSectionProps) {
  const bg = surface ? 'var(--lp-surface)' : 'var(--lp-bg)'
  const borderStyle =
    borderY ?
      { background: bg, borderTop: '1px solid var(--lp-border)', borderBottom: '1px solid var(--lp-border)' }
    : borderTop ? { background: bg, borderTop: '1px solid var(--lp-border)' }
    : { background: bg }

  return (
    <section style={borderStyle} className={`px-6 py-24 ${className}`} {...rest}>
      {children}
    </section>
  )
}

// ── Inner content container ───────────────────────────────────────────────
interface LPContainerProps extends HTMLAttributes<HTMLDivElement> {
  /** sm = max-w-3xl, md = max-w-5xl (default), lg = max-w-6xl */
  size?: 'sm' | 'md' | 'lg'
  children: ReactNode
}

const containerWidth: Record<'sm' | 'md' | 'lg', string> = {
  sm: 'max-w-3xl',
  md: 'max-w-5xl',
  lg: 'max-w-6xl'
}

export function LPContainer({ size = 'md', className = '', children, ...rest }: LPContainerProps) {
  return (
    <div className={`mx-auto ${containerWidth[size]} ${className}`} {...rest}>
      {children}
    </div>
  )
}

// ── Section eyebrow label ─────────────────────────────────────────────────
interface LPEyebrowProps {
  children: ReactNode
  className?: string
}

export function LPEyebrow({ children, className = '' }: LPEyebrowProps) {
  return (
    <motion.div
      initial={{ opacity: 0 }}
      whileInView={{ opacity: 1 }}
      viewport={{ once: true }}
      transition={{ duration: 0.4 }}
      className={`mb-2 ${className}`}
    >
      <span className="text-lp-muted font-mono text-sm uppercase tracking-widest">{children}</span>
    </motion.div>
  )
}

// ── Section heading (h2) ──────────────────────────────────────────────────
interface LPSectionHeadingProps {
  children: ReactNode
  className?: string
}

export function LPSectionHeading({ children, className = '' }: LPSectionHeadingProps) {
  return (
    <motion.h2
      initial={{ opacity: 0, y: 16 }}
      whileInView={{ opacity: 1, y: 0 }}
      viewport={{ once: true }}
      transition={{ duration: 0.4, delay: 0.05 }}
      className={`text-lp-text font-mono text-4xl font-bold sm:text-xl ${className}`}
    >
      {children}
    </motion.h2>
  )
}

// ── Buttons ───────────────────────────────────────────────────────────────
interface LPBtnProps {
  href?: string
  onClick?: () => void
  children: ReactNode
  className?: string
}

// Filled accent button
export function LPPrimaryBtn({ href, children, className = '', onClick }: LPBtnProps) {
  const cls = `bg-lp-accent text-lp-bg font-mono text-sm font-bold uppercase tracking-wide transition-colors hover:bg-lp-accent-hover ${className}`
  if (href)
    return (
      <Link href={href} className={`inline-block px-7 py-3 ${cls}`}>
        {children}
      </Link>
    )
  return (
    <button onClick={onClick} className={`px-7 py-3 ${cls}`}>
      {children}
    </button>
  )
}

// Outlined ghost button
export function LPGhostBtn({ href, children, className = '', onClick }: LPBtnProps) {
  const cls = `border-lp-border text-lp-muted hover:text-lp-text hover:border-lp-muted border font-mono text-sm font-bold uppercase tracking-wide transition-colors ${className}`
  if (href)
    return (
      <Link href={href} className={`inline-block px-7 py-3 ${cls}`}>
        {children}
      </Link>
    )
  return (
    <button onClick={onClick} className={`px-7 py-3 ${cls}`}>
      {children}
    </button>
  )
}

// Tab switcher button (used in code panels)
interface LPTabBtnProps {
  active: boolean
  onClick: () => void
  children: ReactNode
}

export function LPTabBtn({ active, onClick, children }: LPTabBtnProps) {
  return (
    <button
      onClick={onClick}
      className={[
        'inline-flex items-center gap-1.5 px-3 py-1 font-mono text-sm uppercase tracking-wider transition-colors',
        active ? 'bg-lp-accent text-lp-bg' : 'text-lp-muted hover:text-lp-text'
      ].join(' ')}
    >
      {children}
    </button>
  )
}

// ── Accent left-border callout card ──────────────────────────────────────
interface LPAccentCardProps {
  label: string
  body: string
  delay?: number
}

export function LPAccentCard({ label, body, delay = 0 }: LPAccentCardProps) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 16 }}
      whileInView={{ opacity: 1, y: 0 }}
      viewport={{ once: true }}
      transition={{ duration: 0.35, delay }}
      className="border-lp-accent border-l-2 pl-4"
    >
      <p className="text-lp-accent mb-1 font-mono text-sm font-bold">✓&nbsp; {label}</p>
      <p className="text-lp-muted font-sans text-sm leading-relaxed">{body}</p>
    </motion.div>
  )
}

// ── Shared LP switch (squared, LP-token-aware) ────────────────────────────
interface LPSwitchProps {
  checked: boolean
  onCheckedChange: (checked: boolean) => void
  'aria-label'?: string
}

export function LPSwitch({ checked, onCheckedChange, 'aria-label': ariaLabel }: LPSwitchProps) {
  return (
    <SwitchPrimitives.Root
      checked={checked}
      onCheckedChange={onCheckedChange}
      aria-label={ariaLabel}
      className="border-lp-border relative inline-flex h-5 w-9 shrink-0 cursor-pointer items-center border transition-colors focus:outline-none"
      style={{ borderRadius: 0, background: checked ? 'var(--lp-accent)' : 'var(--lp-border)' }}
    >
      <SwitchPrimitives.Thumb
        className="pointer-events-none block h-3 w-3 transition-transform data-[state=checked]:translate-x-5 data-[state=unchecked]:translate-x-1"
        style={{ borderRadius: 0, background: checked ? 'var(--lp-bg)' : 'var(--lp-text)' }}
      />
    </SwitchPrimitives.Root>
  )
}
