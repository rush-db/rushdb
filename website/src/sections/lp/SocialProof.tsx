import { motion } from 'framer-motion'
import Link from 'next/link'
import { socials } from '~/config/urls'
import { LPContainer } from '~/components/lp/ui'

const TRUST = [
  {
    label: '★  GitHub',
    href: socials.github
  },
  {
    label: 'Open source — Apache 2.0',
    href: socials.github
  },
  {
    label: 'MCP server available',
    href: 'https://github.com/rush-db/rushdb-mcp'
  }
]

export function LPSocialProof() {
  return (
    <section style={{ background: 'var(--lp-bg)' }} className="px-6 py-20">
      <LPContainer>
        <motion.div
          initial={{ opacity: 0 }}
          whileInView={{ opacity: 1 }}
          viewport={{ once: true }}
          transition={{ duration: 0.4 }}
          className="mb-10 flex flex-wrap items-center justify-center gap-10"
        >
          {TRUST.map(({ label, href }) => (
            <Link
              key={label}
              href={href}
              target="_blank"
              rel="noopener noreferrer"
              className="text-lp-muted hover:text-lp-text font-mono text-sm uppercase tracking-wider transition-colors"
            >
              {label}
            </Link>
          ))}
        </motion.div>

        <motion.p
          initial={{ opacity: 0 }}
          whileInView={{ opacity: 1 }}
          viewport={{ once: true }}
          transition={{ duration: 0.4, delay: 0.15 }}
          className="text-lp-muted text-center font-mono text-sm tracking-wide"
        >
          Self-hostable. Your data never leaves your infrastructure.
        </motion.p>
      </LPContainer>
    </section>
  )
}
