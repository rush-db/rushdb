import type { ComponentPropsWithoutRef } from 'react'

import { cn } from '~/lib/utils'

export function PageHeader({
  children,
  contained,
  className
}: ComponentPropsWithoutRef<'header'> & { contained?: boolean }) {
  return (
    <header
      className={cn(
        'flex min-h-[84px] flex-shrink-0 flex-wrap items-center justify-between gap-3 pt-10',
        { 'p-5': !contained, 'container py-5': contained },
        className
      )}
    >
      {children}
    </header>
  )
}

export function PageTitle({
  children,
  className
}: ComponentPropsWithoutRef<'div'>) {
  return (
    <h4
      className={cn(
        'whitespace-nowrap text-2xl font-bold leading-none text-content',
        className
      )}
    >
      {children}
    </h4>
  )
}

export function PageContent({
  children,
  className,
  contained
}: ComponentPropsWithoutRef<'div'> & { contained?: boolean }) {
  return (
    <div
      className={cn(
        'flex flex-col gap-10 pb-5',
        { container: contained, 'px-5': !contained },
        className
      )}
    >
      {children}
    </div>
  )
}
