import { motion } from 'framer-motion'
import { AnimatedJSON } from '~/components/lp/AnimatedJSON'
import { HeroGridCanvas } from '~/components/lp/HeroGridCanvas'
import { links } from '~/config/urls'
import { LPPrimaryBtn, LPGhostBtn } from '~/components/lp/ui'

const fadeUp = (delay = 0) => ({
  initial: { opacity: 0, y: 20 },
  animate: { opacity: 1, y: 0 },
  transition: { duration: 0.5, delay, ease: 'easeOut' }
})

export function LPHero() {
  return (
    <section className="bg-lp-bg relative flex flex-1 flex-col items-center justify-center px-4 pb-0 pt-32 md:pt-4">
      {/* Interactive grid background */}
      <HeroGridCanvas />

      {/* Text content — constrained width */}
      <div className="relative z-10 mx-auto mb-16 w-full max-w-3xl text-center">
        {/* Eyebrow */}
        <motion.div {...fadeUp(0)}>
          <span className="text-lp-muted mb-8 inline-block font-mono text-sm uppercase tracking-widest">
            [ RUSHDB 2.0 — NOW WITH NATIVE VECTOR SEARCH ]
          </span>
        </motion.div>

        {/* Headline */}
        <motion.h1 {...fadeUp(0.15)} className="mb-6">
          <span className="text-lp-text block font-mono text-4xl font-bold leading-tight sm:text-xl">
            Memory for agents
          </span>
          <span className="text-lp-text block font-mono text-4xl font-bold leading-tight sm:text-xl">
            and apps. Instant.
          </span>
        </motion.h1>

        {/* Subline */}
        <motion.p
          {...fadeUp(0.3)}
          className="text-lp-muted mx-auto mb-10 max-w-xl font-sans text-lg leading-relaxed md:text-sm"
        >
          Push any JSON or CSV. Get graph relationships <br className="md:inline" />
          and vector search automatically.
          <br />
          No schema. No pipeline. No configuration.
        </motion.p>

        {/* CTAs */}
        <motion.div {...fadeUp(0.45)} className="flex flex-wrap items-center justify-center gap-4">
          <LPPrimaryBtn href={links.app}>Start free</LPPrimaryBtn>
          <LPGhostBtn href={links.docs}>View docs →</LPGhostBtn>
        </motion.div>
      </div>

      {/* AnimatedJSON — full content width, bleeds into next section */}
      <motion.div
        initial={{ opacity: 0, y: 40 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.7, delay: 0.6, ease: 'easeOut' }}
        className="relative z-10 mx-auto -mb-28 w-full max-w-6xl px-6 md:px-0"
      >
        <AnimatedJSON />
      </motion.div>
    </section>
  )
}
