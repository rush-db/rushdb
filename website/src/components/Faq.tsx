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
        <p ref={ref} className="py-5">
          {answer}
        </p>
      </motion.div>
    </li>
  )
}

export const Faq = ({
  items,
  className
}: {
  items: { question: string; answer: string }[]
  className?: string
}) => {
  const [activeIndex, setActiveIndex] = useState(0)

  return (
    <ul className={classNames('flex flex-col gap-5', className)}>
      {items.map((item, index) => (
        <FaqItem key={index} open={activeIndex === index} onClick={() => setActiveIndex(index)} {...item} />
      ))}
    </ul>
  )
}
