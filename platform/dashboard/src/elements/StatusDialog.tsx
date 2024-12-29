import type { VariantProps } from 'class-variance-authority'
import type { ComponentPropsWithoutRef, ReactNode } from 'react'

import { cva } from 'class-variance-authority'
import { AlertTriangle, Check, Info } from 'lucide-react'

import type { DeepNonNullable, DeepRequired } from '~/types'

import { Button } from '~/elements/Button'
import { Dialog, DialogFooter, DialogTitle } from '~/elements/Dialog'
import { cn } from '~/lib/utils'

const statusIconVariants = cva(
  'grid h-16 w-16 place-items-center rounded-circle',
  {
    defaultVariants: { variant: 'success' },
    variants: {
      variant: {
        success:
          'bg-gradient-to-br from-accent/10 to-accent-hover/30 text-accent',
        error:
          'bg-gradient-to-br from-danger/10 to-danger-hover/30 text-danger',
        info: 'bg-gradient-to-br from-badge-blue/10 to-badge-blue/30 text-badge-blue'
      }
    }
  }
)

const mapStatusIcons = {
  success: Check,
  error: AlertTriangle,
  info: Info
} as const

function StatusIcon({
  className,
  variant = 'success',
  ...props
}: ComponentPropsWithoutRef<'div'> &
  DeepNonNullable<VariantProps<typeof statusIconVariants>>) {
  const Icon = mapStatusIcons[variant]

  return (
    <div className={cn(statusIconVariants({ variant }), className)} {...props}>
      <Icon size={42} />
    </div>
  )
}

const buttonVariantMap = {
  success: 'accent',
  error: 'danger',
  info: 'info'
} as const

export function StatusDialog({
  className,
  title,
  description,
  variant = 'success',
  secondaryActions,
  primaryText,
  primaryAction,
  ...props
}: ComponentPropsWithoutRef<typeof Dialog> & {
  description?: string
  primaryAction: () => void
  primaryText: string
  secondaryActions?: ReactNode
  title: string
} & DeepNonNullable<VariantProps<typeof statusIconVariants>>) {
  const buttonVariant = buttonVariantMap[variant]

  return (
    <Dialog
      className={cn(className, 'min-h-80 items-center justify-items-center')}
      {...props}
    >
      <div className="my-auto grid h-full place-items-center gap-1 text-center">
        <StatusIcon variant={variant} />

        <DialogTitle className="mt-1 justify-self-center">{title}</DialogTitle>

        <p className=" text-content2">{description}</p>
      </div>

      <DialogFooter className="flex w-full shrink-0 flex-col bg-inherit">
        <Button onClick={primaryAction} variant={buttonVariant}>
          {primaryText}
        </Button>
        {secondaryActions}
      </DialogFooter>
    </Dialog>
  )
}
