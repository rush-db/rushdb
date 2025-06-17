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
        "Unlike Firebase's document hierarchies or Supabase's rigid schemas, RushDB offers a zero-config graph database that automatically normalizes your data. You can push JSON directly without planning your data structure in advance, and query across relationships naturally without complex joins."
    },
    {
      question: 'Can I use RushDB for AI applications and LLM outputs?',
      answer:
        "Absolutely! RushDB is designed for the AI era with seamless JSON storage for LLM outputs, automatic relationship detection, and graph-based querying that's perfect for RAG applications, embeddings storage, and knowledge graphs. Our auto-normalization feature is particularly valuable for handling the varied structures of AI-generated content."
    },
    {
      question: 'How much data preparation do I need before using RushDB?',
      answer:
        "Zero. RushDB's core value is eliminating data preparation overhead. Just push your JSON or CSV as-is, and RushDB automatically normalizes, connects, and indexes your data with proper relationships and types. This means you can start building features immediately instead of planning database schemas."
    },
    {
      question: "What's the performance like for real-world applications?",
      answer:
        'RushDB processes data at ~0.25ms per record with ACID transaction support, handling payloads up to 32MB. It can manage 10,000+ e-commerce products, 100,000+ financial transactions, or 1,000,000+ API logs in a single operation, making it production-ready for demanding applications.'
    },
    {
      question: 'Can I self-host RushDB or do I have to use the cloud version?',
      answer:
        'Both options are available. You can self-host using our Docker container with your Neo4j instance, or use RushDB Cloud which offers 2 free projects forever with no maintenance required. For teams that want to focus on building rather than infrastructure, our cloud option eliminates all database management concerns.'
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
