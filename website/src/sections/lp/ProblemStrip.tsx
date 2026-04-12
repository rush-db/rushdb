import { motion } from 'framer-motion'
import { BrainCircuit, DatabaseZap, Network } from 'lucide-react'
import { fadeUp } from '~/components/lp/ui'

const PROBLEMS = [
  {
    icon: BrainCircuit,
    pain: 'Agent needs memory across sessions',
    old: 'Redis + Pinecone + Neo4j + glue code'
  },
  {
    icon: DatabaseZap,
    pain: 'App stores evolving unstructured data',
    old: 'Design schema → write migrations → repeat'
  },
  {
    icon: Network,
    pain: 'RAG needs context beyond flat chunks',
    old: 'Hand-craft relationships in a vector store'
  }
]

export function LPProblemStrip() {
  return (
    <section
      style={{
        background: 'var(--lp-surface)',
        borderTop: '1px solid var(--lp-border)',
        borderBottom: '1px solid var(--lp-border)'
      }}
    >
      <div className="mx-auto max-w-5xl px-6 pb-20 pt-44">
        <div className="grid grid-cols-3 gap-8 md:grid-cols-1">
          {PROBLEMS.map(({ icon: Icon, pain, old }, i) => (
            <motion.div key={i} {...fadeUp(i * 0.1)} className="flex flex-col gap-4">
              <Icon size={20} className="text-lp-muted" />
              <div>
                <p className="text-lp-text mb-2 font-mono text-base font-bold">{pain}</p>
                <div className="bg-lp-border mb-3 h-px w-8" />
                <p className="text-lp-muted font-mono text-sm line-through">{old}</p>
              </div>
            </motion.div>
          ))}
        </div>

        <motion.p
          initial={{ opacity: 0 }}
          whileInView={{ opacity: 1 }}
          viewport={{ once: true }}
          transition={{ duration: 0.5, delay: 0.4 }}
          className="text-lp-accent mt-16 text-center font-mono text-sm"
        >
          "You shouldn't need three databases and a pipeline for this."
        </motion.p>
      </div>
    </section>
  )
}
