import { ReactNode } from "react"
import { LetterTypingText } from "~/components/LetterTypingText"

export function UsageScenario({
  title,
  description,
  subtitle,
  example,
}: {
  title: ReactNode
  subtitle: ReactNode
  description: ReactNode
  example: ReactNode
}) {
  return (
    <div className="grid grid-cols-2 content-start justify-items-center gap-10 py-16 md:grid-cols-1">
      <div className="flex first:pt-0 pt-16 flex-col items-start col-span-1 gap-3 justify-self-start sm:pt-0">
        {subtitle}

        {typeof title === "string" ? (
          <LetterTypingText as="h3" className="typography-xl" animateInView>
            {title}
          </LetterTypingText>
        ) : (
          <h3 className="typography-xl">{title}</h3>
        )}

        <p className="typography-base text-content2">{description}</p>
      </div>

      {example}
    </div>
  )
}
