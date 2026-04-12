import { motion } from 'framer-motion'
import { links } from '~/config/urls'
import { LPPrimaryBtn } from '~/components/lp/ui'

export function LPFooterCTA() {
  return (
    <section style={{ background: 'var(--lp-bg)' }} className="px-6 py-32">
      <div className="mx-auto max-w-3xl text-center">
        <motion.h2
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.5 }}
        >
          <span className="text-lp-text block font-mono text-4xl font-bold leading-tight sm:text-xl">
            Give your agent
          </span>
          <span className="text-lp-text block font-mono text-4xl font-bold leading-tight sm:text-xl">
            a memory.
          </span>
        </motion.h2>

        <motion.div
          initial={{ opacity: 0, y: 16 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.45, delay: 0.15 }}
          className="mt-10"
        >
          <LPPrimaryBtn href={links.app} className="px-8 py-4">
            Get API key
          </LPPrimaryBtn>
        </motion.div>
      </div>
    </section>
  )
}
