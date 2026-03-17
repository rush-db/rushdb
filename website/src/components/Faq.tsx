import classNames from 'classnames'
import { motion } from 'framer-motion'
import { Plus } from 'lucide-react'
import { useState } from 'react'
import { IconButton } from '~/components/IconButton'
import { useSize } from '~/hooks/useSize'

function FaqItem({
  question,
  answer,
  open,
  onClick
}: {
  question: string
  answer: string
  open: boolean
  onClick?: () => void
}) {
  const { height, ref } = useSize()

  return (
    <li>
      <h5
        className={classNames('typography-lg flex min-h-10 justify-between', {
          'hover:text-content2 cursor-pointer transition-colors': !open
        })}
        onClick={onClick}
      >
        {question}

        <IconButton
          aria-label="Toggle answer"
          variant="primaryText"
          className={classNames('pointer-events-none transition-transform', open ? 'rotate-45' : '')}
        >
          <Plus />
        </IconButton>
      </h5>

      <motion.div
        initial={{ height: 0 }}
        animate={{ height: open ? height : '0px' }}
        exit={{ height: 0 }}
        transition={{ type: 'spring', duration: 0.4, bounce: 0 }}
        className={classNames('text-content2 overflow-hidden text-base', {})}
      >
        <p ref={ref} className="text-content3 mb2 text-md py-5 md:text-base">
          {answer}
        </p>
      </motion.div>
    </li>
  )
}

export const Faq = ({
  items = [
    {
      question: 'How is RushDB different from Firebase or Supabase?',
      answer:
        "Unlike Firebase's document hierarchies or Supabase's rigid schemas, RushDB automatically normalizes any JSON into a connected knowledge graph — no schema design required. Push data and query it instantly. Pricing is transparent: standard reads and queries are always free. You pay only for writes and compute-intensive operations (vector search, raw Cypher), measured in Knowledge Units."
    },
    {
      question: 'Can I use RushDB for AI applications and LLM outputs?',
      answer:
        'Absolutely. RushDB is purpose-built for AI workloads — push LLM outputs, structured embeddings, or raw event data and query across them relationally. Auto-normalization handles the varied structures of AI-generated content, and graph-based querying is perfect for RAG pipelines and knowledge graph construction.'
    },
    {
      question: 'How much data preparation do I need before using RushDB?',
      answer:
        'Zero. Just push your JSON as-is and RushDB automatically decomposes, connects, and indexes your data with proper relationships and types. You can start querying within the same request — no schema planning, no migrations, no boilerplate.'
    },
    {
      question: 'How does billing work — what is a Knowledge Unit (KU)?',
      answer:
        'A Knowledge Unit (KU) is the atomic measure of structured knowledge created or maintained by RushDB. A record with 10 properties costs 10 KU. Nested objects are decomposed into linked records, each contributing its own KU. Standard reads and queries are always free. Compute-intensive operations (vector search, raw Cypher, deep traversals) consume a small amount of KU. The Free plan includes 100K KU/month — enough for ~3,000 records with 10 fields each.'
    },
    {
      question: 'Can I self-host RushDB or do I have to use the cloud version?',
      answer:
        'Both options are available. Self-host using our Docker container with your own Neo4j instance — no limits, no billing. RushDB Cloud starts free: 100K KU/month and 2 projects, no credit card required. Paid plans begin at $29/month with overage billing so you only pay for what you use.'
    }
  ],
  className
}: {
  items?: { question: string; answer: string }[]
  className?: string
}) => {
  const [activeIndex, setActiveIndex] = useState(0)

  return (
    <>
      <h4 className="py-8 text-xl font-bold leading-tight">FAQ</h4>

      <ul className={classNames('flex flex-col gap-5', className)}>
        {items.map((item, index) => (
          <FaqItem key={index} open={activeIndex === index} onClick={() => setActiveIndex(index)} {...item} />
        ))}
      </ul>
    </>
  )
}
