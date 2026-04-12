import { motion } from 'framer-motion'
import { Cpu, GitBranch, Lock, Zap, Server, Layers } from 'lucide-react'
import { LPSection, LPContainer, LPEyebrow, LPSectionHeading } from '~/components/lp/ui'

const FEATURES = [
  {
    icon: Zap,
    name: 'Managed Embeddings',
    desc: 'Index any string property once. Every write is auto-embedded server-side — new records instantly, existing ones backfilled. Prefer your own model? Pass pre-computed vectors instead.'
  },
  {
    icon: Layers,
    name: 'Vector + Graph',
    desc: 'Semantic similarity and graph traversal compose in one query. Filter by relationships, rank by meaning — no separate stores to sync.'
  },
  {
    icon: Cpu,
    name: 'MCP-native',
    desc: 'Plug into Claude, Cursor, or any MCP-compatible agent out of the box.'
  },
  {
    icon: Lock,
    name: 'ACID transactions',
    desc: "Memory that doesn't corrupt under concurrent writes. Neo4j under the hood."
  },
  {
    icon: Server,
    name: 'Self-host or Cloud',
    desc: 'Docker + your Neo4j, or managed cloud. Full data ownership if you need it.'
  },
  {
    icon: GitBranch,
    name: 'Unified Query API',
    desc: 'One JSON query shape for graph, vector, aggregation, and introspection.'
  }
]

export function LPFeatureProof() {
  return (
    <LPSection surface borderY>
      <LPContainer>
        <LPEyebrow>Features</LPEyebrow>
        <LPSectionHeading className="mb-14">What's in the box.</LPSectionHeading>

        <div className="grid grid-cols-3 gap-8 sm:grid-cols-1 md:grid-cols-2">
          {FEATURES.map(({ icon: Icon, name, desc }, i) => (
            <motion.div
              key={i}
              initial={{ opacity: 0, y: 20 }}
              whileInView={{ opacity: 1, y: 0 }}
              viewport={{ once: true }}
              transition={{ duration: 0.35, delay: i * 0.07 }}
              className="border-lp-border flex flex-col gap-3 border p-6"
            >
              <Icon size={16} className="text-lp-muted" />
              <p className="text-lp-text font-mono text-sm">{name}</p>
              <p className="text-lp-muted font-sans text-sm leading-relaxed">{desc}</p>
            </motion.div>
          ))}
        </div>
      </LPContainer>
    </LPSection>
  )
}
