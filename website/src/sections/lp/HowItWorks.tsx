import { motion } from 'framer-motion'
import { LPSection, LPContainer, LPEyebrow, LPSectionHeading } from '~/components/lp/ui'

const STEPS = [
  {
    num: '01',
    title: 'Push data',
    body: 'Any JSON or CSV. Any shape. No schema required. No planning. RushDB accepts it as-is.'
  },
  {
    num: '02',
    title: 'Instant structure',
    body: 'Graph relationships auto-detected. Vectors indexed if embeddings present, or attach them separately via BYOV.'
  },
  {
    num: '03',
    title: 'Query both',
    body: 'One unified API. Semantic similarity + graph traversal in a single call.'
  }
]

export function LPHowItWorks() {
  return (
    <LPSection>
      <LPContainer>
        <LPEyebrow className="mb-4">How it works</LPEyebrow>
        <LPSectionHeading className="mb-16">Three steps.</LPSectionHeading>

        <div className="grid grid-cols-3 gap-10 md:grid-cols-1 md:gap-12">
          {STEPS.map(({ num, title, body }, i) => (
            <motion.div
              key={i}
              initial={{ opacity: 0, x: 20 }}
              whileInView={{ opacity: 1, x: 0 }}
              viewport={{ once: true }}
              transition={{ duration: 0.4, delay: i * 0.15 }}
              className="relative"
            >
              {/* Large faint number behind */}
              <span
                className="text-lp-border pointer-events-none absolute -left-2 -top-4 select-none font-mono font-bold leading-none"
                style={{ fontSize: '5rem' }}
                aria-hidden
              >
                {num}
              </span>

              <div className="relative z-10 pt-8">
                <h3 className="text-lp-text mb-3 font-mono text-sm">{title}</h3>
                <div className="bg-lp-border mb-4 h-px w-full" />
                <p className="text-lp-muted font-sans text-sm leading-relaxed">{body}</p>
              </div>
            </motion.div>
          ))}
        </div>
      </LPContainer>
    </LPSection>
  )
}
