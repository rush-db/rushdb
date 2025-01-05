import cx from 'classnames'
import { ComponentPropsWithoutRef, ElementType, forwardRef, PropsWithChildren } from 'react'

import { motion } from 'framer-motion'
import { LetterTypingText } from '~/components/LetterTypingText'

interface SectionProps {
  className?: string
  as?: ElementType<any>

  [key: string]: unknown
}

export const Section = forwardRef<HTMLElement, PropsWithChildren<SectionProps>>(
  ({ className, children, as: As = 'section', ...props }, ref) => {
    return (
      <motion.div initial={{ opacity: 0 }} whileInView={{ opacity: 1 }} viewport={{ once: true }}>
        {/*  @ts-ignore*/}
        <As
          ref={ref}
          className={cx(
            'section flex flex-col gap-12 py-20',
            {
              'bg-fill': props['data-theme'] === 'dark',
              'bg-background-light': props['data-theme'] === 'light'
            },
            className as string
          )}
          {...props}
        >
          {children}
        </As>
      </motion.div>
    )
  }
)

Section.displayName = 'Section'

export function SectionHeader({ className, ...props }: ComponentPropsWithoutRef<'div'>) {
  return <div className={cx('flex flex-col gap-5', className)} {...props} />
}

export function SectionTitle({
  className,
  ...props
}: ComponentPropsWithoutRef<'div'> & { children: string }) {
  return (
    <LetterTypingText
      as="h2"
      className={cx('typography-3xl mb-0 md:text-2xl', className)}
      animateInView
      {...props}
    />
  )
}

export function SectionSubtitle({ className, ...props }: ComponentPropsWithoutRef<'div'>) {
  return <p className={cx('typography-base text-content2', className)} {...props} />
}
