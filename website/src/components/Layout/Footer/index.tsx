import { Linkedin } from 'lucide-react'
import Link from 'next/link'
import { GitHub } from '~/components/Icons/GitHub'
import { IconX } from '~/components/Layout/IconX'
import { LPLogo } from '~/components/lp/LPLogo'
import { links, socials } from '~/config/urls'

export function Footer() {
  return (
    <footer
      style={{ background: 'var(--lp-bg)', borderTop: '1px solid var(--lp-border)' }}
      className="px-6 py-12"
    >
      <div className="mx-auto max-w-5xl">
        <div className="flex flex-wrap items-start justify-between gap-8">
          {/* Left: logo + nav + legal */}
          <div className="flex flex-col gap-6">
            <div className="flex flex-wrap items-center gap-8">
              <Link href="/" aria-label="Home page" className="flex items-center gap-2">
                <LPLogo width={20} height={20} className="text-lp-text" />
                <span className="text-lp-text font-mono text-sm tracking-widest">rushdb</span>
              </Link>

              <nav className="flex flex-wrap gap-6">
                {[
                  { label: 'Docs', href: links.docs },
                  { label: 'GitHub', href: socials.github },
                  { label: 'Blog', href: socials.blog },
                  { label: 'Pricing', href: links.pricing }
                ].map(({ label, href }) => (
                  <Link
                    key={label}
                    href={href}
                    className="text-lp-muted hover:text-lp-text font-mono text-sm uppercase tracking-wider transition-colors"
                  >
                    {label}
                  </Link>
                ))}
              </nav>
            </div>

            <div className="flex flex-wrap items-center gap-6">
              <p className="text-lp-muted font-mono text-sm">
                © {new Date().getFullYear()}, Collect Software Inc.
              </p>
              <nav className="flex flex-wrap gap-6">
                {[
                  { label: 'Privacy Policy', href: '/privacy-policy' },
                  { label: 'Terms of Service', href: '/terms-of-service' },
                  { label: 'Cookie Policy', href: '/cookie-policy' }
                ].map(({ label, href }) => (
                  <Link
                    key={label}
                    href={href}
                    className="text-lp-muted hover:text-lp-text font-mono text-sm transition-colors"
                  >
                    {label}
                  </Link>
                ))}
              </nav>
            </div>
          </div>

          {/* Right: social icons */}
          <div className="flex items-center gap-5">
            <Link
              href={socials.x}
              target="__blank"
              rel="noopener noreferrer"
              aria-label="X (Formerly Twitter)"
            >
              <IconX className="text-lp-muted hover:text-lp-text h-4 w-4 transition-colors" />
            </Link>
            <Link href={socials.github} target="__blank" rel="noopener noreferrer" aria-label="GitHub">
              <GitHub className="text-lp-muted hover:text-lp-text h-4 w-4 transition-colors" />
            </Link>
            <Link href={socials.linkedIn} target="__blank" rel="noopener noreferrer" aria-label="LinkedIn">
              <Linkedin
                className="text-lp-muted hover:text-lp-text h-4 w-4 transition-colors"
                strokeWidth={1.5}
              />
            </Link>
          </div>
        </div>
      </div>
    </footer>
  )
}
