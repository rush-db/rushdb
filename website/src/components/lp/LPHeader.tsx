'use client'
import { useState } from 'react'
import Link from 'next/link'
import { Menu, X } from 'lucide-react'
import { GitHub } from '~/components/Icons/GitHub'
import { IconX } from '~/components/Layout/IconX'
import { LPLogo } from '~/components/lp/LPLogo'
import { ThemeToggle } from '~/components/lp/ThemeToggle'
import { links, socials } from '~/config/urls'

const NAV = [
  { label: 'Docs', href: links.docs },
  { label: 'Blog', href: socials.blog },
  { label: 'Pricing', href: links.pricing }
]

export function LPHeader() {
  const [mobileOpen, setMobileOpen] = useState(false)

  return (
    <header
      className="fixed top-0 z-30 w-full"
      style={{
        background: 'var(--lp-header-bg)',
        borderBottom: '1px solid var(--lp-border)',
        backdropFilter: 'blur(12px)'
      }}
    >
      <div className="mx-auto flex h-16 max-w-6xl items-center justify-between px-6">
        {/* Logo + wordmark + Desktop Nav */}
        <div className="flex items-center gap-8 sm:gap-0">
          <Link href="/" className="group flex items-center gap-2">
            <LPLogo
              width={28}
              height={28}
              className="text-lp-text group-hover:text-lp-accent transition-colors"
            />
            <span className="text-lp-text group-hover:text-lp-accent font-mono text-sm font-bold transition-colors">
              rushdb
            </span>
          </Link>

          <nav className="flex items-center gap-6 sm:hidden">
            {NAV.map(({ label, href }) => (
              <Link
                key={label}
                href={href}
                className="text-lp-muted hover:text-lp-text font-mono text-sm uppercase tracking-wider transition-colors"
              >
                {label}
              </Link>
            ))}
            <Link
              href={socials.github}
              target="_blank"
              rel="noopener noreferrer"
              className="text-lp-muted hover:text-lp-text transition-colors"
              aria-label="GitHub"
            >
              <GitHub width={18} height={18} className="text-current" />
            </Link>
          </nav>
        </div>

        {/* CTA */}
        <div className="flex items-center gap-6">
          <ThemeToggle />
          <Link
            href="https://app.rushdb.com"
            className="text-lp-muted hover:text-lp-text font-mono text-sm uppercase tracking-wider transition-colors sm:hidden"
          >
            Sign In
          </Link>
          <Link
            href={links.app}
            className="bg-lp-accent text-lp-bg hover:bg-lp-accent-hover px-4 py-2 font-mono text-sm font-bold uppercase tracking-wide transition-colors sm:hidden"
          >
            Start Free
          </Link>
          {/* Mobile hamburger */}
          <button
            className="hidden items-center justify-center sm:flex"
            onClick={() => setMobileOpen((v) => !v)}
            aria-label="Toggle menu"
            style={{ background: 'none', border: 'none', cursor: 'pointer' }}
          >
            {mobileOpen ?
              <X size={20} className="text-lp-text" />
            : <Menu size={20} className="text-lp-text" />}
          </button>
        </div>
      </div>

      {/* Mobile drawer */}
      {mobileOpen && (
        <div
          style={{ background: 'var(--lp-header-bg)', borderBottom: '1px solid var(--lp-border)' }}
          className="hidden px-6 pb-6 sm:block"
        >
          <nav className="flex flex-col gap-4 pt-4">
            {NAV.map(({ label, href }) => (
              <Link
                key={label}
                href={href}
                onClick={() => setMobileOpen(false)}
                className="text-lp-muted hover:text-lp-text font-mono text-sm uppercase tracking-wider transition-colors"
              >
                {label}
              </Link>
            ))}
            <div className="flex items-center gap-4 pt-2">
              <Link
                href={socials.github}
                target="_blank"
                rel="noopener noreferrer"
                className="text-lp-muted hover:text-lp-text transition-colors"
                aria-label="GitHub"
              >
                <GitHub width={18} height={18} className="text-current" />
              </Link>
              <Link
                href={socials.x}
                target="_blank"
                rel="noopener noreferrer"
                className="text-lp-muted hover:text-lp-text transition-colors"
                aria-label="X"
              >
                <IconX className="h-[18px] w-[18px]" />
              </Link>
            </div>
            <div className="flex gap-3 pt-2">
              <Link
                href="https://app.rushdb.com"
                className="text-lp-muted hover:text-lp-text font-mono text-sm uppercase tracking-wider transition-colors"
              >
                Sign In
              </Link>
              <Link
                href={links.app}
                className="bg-lp-accent text-lp-bg hover:bg-lp-accent-hover px-4 py-2 font-mono text-sm font-bold uppercase tracking-wide transition-colors"
              >
                Start Free
              </Link>
            </div>
          </nav>
        </div>
      )}
    </header>
  )
}
