import classNames from "classnames"
import { motion } from "framer-motion"
import { Plus } from "lucide-react"
import { useState } from "react"
import { IconButton } from "~/components/IconButton"
import { useSize } from "~/hooks/useSize"

function FaqItem({
  question,
  answer,
  open,
  onClick,
}: {
  question: string
  answer: string
  open: boolean
  onClick?: () => void
}) {
  const { height, ref } = useSize()

  return (
    <li>
      <h3
        className={classNames(
          "typography-xl min-h-16 md:min-h-11 flex justify-between ",
          {
            "cursor-pointer hover:text-content2 transition-colors": !open,
          },
        )}
        onClick={onClick}
      >
        {question}

        <IconButton
          aria-label="Toggle answer"
          variant="primaryText"
          className={classNames(
            "transition-transform pointer-events-none",
            open ? "rotate-45" : "",
          )}
        >
          <Plus />
        </IconButton>
      </h3>

      <motion.div
        initial={{ height: 0 }}
        animate={{ height: open ? height : "0px" }}
        exit={{ height: 0 }}
        transition={{ type: "spring", duration: 0.4, bounce: 0 }}
        className={classNames("text-base text-content2 overflow-hidden", {})}
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
  className,
}: {
  items: { question: string; answer: string }[]
  className?: string
}) => {
  const [activeIndex, setActiveIndex] = useState(0)

  return (
    <ul className={classNames("flex flex-col gap-5", className)}>
      {items.map((item, index) => (
        <FaqItem
          key={index}
          open={activeIndex === index}
          onClick={() => setActiveIndex(index)}
          {...item}
        />
      ))}
    </ul>
  )
}
