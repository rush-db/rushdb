import Link from 'next/link'
import { links } from '~/config/urls'

export const CallToAction = ({
  text = 'Give your agent a memory.',
  buttonText = 'Start building free →',
  description = 'Push any JSON. Get graph relationships and vector search instantly — no schema, no pipeline, no setup.'
}: {
  text?: string
  description?: string
  buttonText?: string
}) => (
  <div
    className="my-12 border p-8 sm:p-5"
    style={{ borderColor: 'var(--lp-border)', background: 'rgba(var(--lp-accent-rgb), 0.03)' }}
  >
    <p className="text-lp-muted mb-1 font-mono text-sm uppercase tracking-widest">RushDB</p>
    <h3 className="text-lp-text mb-3 font-mono text-2xl font-bold sm:text-lg">{text}</h3>
    <p className="text-lp-muted mb-6 font-mono text-sm leading-relaxed">{description}</p>
    <Link
      href={links.app}
      className="bg-lp-accent text-lp-bg hover:bg-lp-accent-hover inline-block px-6 py-3 font-mono text-sm uppercase tracking-wide transition-colors"
    >
      {buttonText}
    </Link>
  </div>
)
