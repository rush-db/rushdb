import type { ReactNode } from 'react'

import { ChevronLeft } from 'lucide-react'

import type { getRoutePath } from '~/lib/router'

import { cn } from '~/lib/utils'

import { IconButton } from './IconButton'

export function Card({
  className,
  ...props
}: TPolymorphicComponentProps<'ul'>) {
  return (
    <article
      className={cn('bg-card flex flex-col rounded-md border', className)}
      {...props}
    />
  )
}

export function CardHeader({
  backHref,
  children,
  className,
  description,
  title,
  ...props
}: TPolymorphicComponentProps<
  'ul',
  {
    backHref?: ReturnType<typeof getRoutePath>
    description?: ReactNode
    title: ReactNode
  }
>) {
  return (
    <header className={cn('p-5', className)} {...props}>
      <div className="flex items-center gap-3 text-base font-semibold">
        {backHref && (
          <IconButton aria-label="Back" as="a" href={backHref} variant="ghost">
            <ChevronLeft />
          </IconButton>
        )}
        {title && <h2 className="font-semibold">{title}</h2>}
      </div>
      {description && <p className="text-content2">{description}</p>}
      {children}
    </header>
  )
}

export function CardBody({
  className,
  ...props
}: TPolymorphicComponentProps<'div'>) {
  return (
    <div className={cn('flex flex-col gap-5 p-5 pt-0', className)} {...props} />
  )
}

export function CardFooter({
  className,
  ...props
}: TPolymorphicComponentProps<'ul'>) {
  return (
    <footer
      className={cn('mt-auto flex justify-end gap-2 border-t p-5', className)}
      {...props}
    />
  )
}
