import { ReactNode } from 'react'
import { cn } from '~/lib/utils'

export function OnboardingStepTitle({
  children,
  className
}: {
  children: ReactNode
  className?: string
}) {
  return (
    <h3 className={cn('col-span-2 text-xl text-content', className)}>
      {children}
    </h3>
  )
}

export function OnboardingStepNumber({
  children,
  className
}: {
  children: ReactNode
  className?: string
}) {
  return (
    <div
      className={cn(
        'flex h-6 w-6 items-center justify-center rounded-full bg-accent/20 sm:h-8 sm:w-8',
        className
      )}
    >
      {children}
    </div>
  )
}

export function OnboardingSubStep({
  children,
  index
}: {
  children: ReactNode
  index: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9
}) {
  return (
    <>
      <OnboardingStepNumber
        className={cn('col-start-1 ', {
          'bg-accent/10': index === 1,
          'bg-accent/20': index === 2,
          'bg-accent/30': index === 3,
          'bg-accent/40': index === 4,
          'bg-accent/50': index === 5,
          'bg-accent/60': index === 6,
          'bg-accent/70': index === 7,
          'bg-accent/80': index === 8,
          'bg-accent/90': index === 9
        })}
      >
        {index}
      </OnboardingStepNumber>
      <h4 className="col-start-2 font-medium leading-6 text-content sm:text-lg sm:leading-8">
        {children}
      </h4>
    </>
  )
}

export function OnboardingStepDescription({
  children,
  className
}: {
  children: ReactNode
  className?: string
}) {
  return <p className={cn('col-span-2 text-content2', className)}>{children}</p>
}

export function OnboardingStepHeader({
  children,
  className,
  sticky
}: {
  children: ReactNode
  sticky?: boolean
  className?: string
}) {
  return (
    <div
      className={cn(
        'flex h-fit flex-col items-start gap-inherit',
        {
          'col-span-1 mb-3 sm:sticky sm:top-12 sm:mb-0': sticky,
          'col-span-full': !sticky
        },
        className
      )}
    >
      {children}
    </div>
  )
}

export function OnboardingStep({
  title,
  content,
  stickyHeader,
  className
}: {
  title: ReactNode
  content?: ReactNode
  stickyHeader?: boolean
  className?: string
}) {
  return (
    <div
      className={cn('grid grid-cols-1 gap-3 py-5 sm:grid-cols-3', className)}
    >
      {title && (
        <OnboardingStepHeader sticky={stickyHeader}>
          {title}
        </OnboardingStepHeader>
      )}

      {content}
    </div>
  )
}
