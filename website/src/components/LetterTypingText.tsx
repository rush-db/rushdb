import { motion } from "framer-motion"

// function useWaypoint() {
//   const ref = useRef<HTMLDivElement>(null);
//   const isInView = useInView(ref, { margin: "400px 0px 0px 0px", once: true });
//   return { ref, isInView } as const;
// }

const sentence = {
  hidden: {
    // opacity: 1,
  },
  visible: {
    // opacity: 1,
    transition: { delay: 0.05, staggerChildren: 0.008 },
  },
}

const letter = {
  hidden: {
    opacity: 0,
    y: 50,
  },
  visible: {
    opacity: 1,
    y: 0,
  },
}

const renderChars = (str: string, className?: string) =>
  str.split("").map((ch, idx) => (
    <motion.span className={className} key={ch + idx} variants={letter}>
      {ch}
    </motion.span>
  ))

export const LetterTypingText = <As extends React.ElementType = "span">({
  animate,
  children: text,
  animateInView = true,
  as,
  ...props
}: TPolymorphicComponentProps<As> & {
  animate?: boolean
  animateInView?: boolean
  children: string
}) => {
  const Element = as ?? "span"

  return (
    <Element {...props} aria-label={text}>
      <motion.span
        variants={sentence}
        initial="hidden"
        animate={animate ? "visible" : "hidden"}
        viewport={{ once: true }}
        {...(animateInView
          ? {
              whileInView: "visible",
            }
          : {})}
        aria-hidden
      >
        {renderChars(text)}
      </motion.span>
    </Element>
  )
}
