import type { VariantProps } from 'class-variance-authority'
import type { ReactNode } from 'react'

import { cva } from 'class-variance-authority'
import { forwardRef, useState } from 'react'

import { useTimeout } from '~/hooks/useTimeout'
import { cn } from '~/lib/utils'

import type { Toast } from './Toast'

import { toast } from './Toast'

export const buttonVariants = cva(
  'cursor-pointer min-w-[70px] inline-grid select-none transition shrink-0 grid-flow-col place-items-center outline-none focus-visible:ring aria-disabled:cursor-not-allowed aria-disabled:opacity-40 font-medium',
  {
    defaultVariants: { size: 'medium', variant: 'ghost' },
    variants: {
      size: {
        large: 'h-[44px] gap-4 px-5 text-lg font-bold rounded-lg',
        medium:
          'h-11 gap-3 [&>svg]:w-[20px] [&>svg]:h-[20px] font-medium rounded-md px-4',
        small:
          'h-9 gap-2 [&>svg]:w-[16px] [&>svg]:h-[16px] rounded-sm px-3 text-sm',
        xsmall:
          'h-7 gap-1 [&>svg]:w-[16px] [&>svg]:h-[16px] rounded-sm px-2 text-xs',
        xxsmall:
          'h-5 gap-1 [&>svg]:w-[16px] [&>svg]:h-[16px] rounded-sm px-1 text-xs'
      },
      variant: {
        success:
          'bg-success text-success-contrast hover:bg-success-hover ring-success-ring',
        accent:
          'bg-accent text-accent-contrast hover:bg-accent-hover focus-visible:bg-accent-focus ring-accent-ring',
        primary:
          'bg-primary ring-primary-ring text-primary-contrast hover:bg-primary-hover',
        info: 'bg-badge-blue ring-badge-blue/10 text-badge-blue-contrast hover:bg-badge-blue/80',
        danger:
          'bg-danger text-danger-contrast hover:bg-danger-hover ring-danger-ring',
        outline:
          'border hover:bg-secondary-hover hover:border-bg-secondary-hover',
        secondary:
          'bg-secondary text-secondary-content hover:bg-secondary-hover ring-secondary-ring',
        ghost:
          'hover:bg-secondary-hover hover:text-secondary-content-hover text-secondary-content',
        dangerGhost: 'hover:bg-danger/20 hover:text-danger-hover text-danger',
        link: 'text-content-2 hover:text-content underline-offset-4 hover:underline',
        inverse: 'hover:bg-fill/30'
      }
    }
  }
)

type TButtonProps = {
  disabled?: boolean
  loading?: boolean
} & VariantProps<typeof buttonVariants>

export const Button: TPolymorphicComponent<TButtonProps, 'button'> = forwardRef(
  (
    {
      as = 'button',
      children,
      className,
      disabled,
      loading,
      size,
      type = 'button',
      variant,
      ...props
    },
    ref
  ) => {
    const Element = as

    return (
      <Element
        className={cn(
          buttonVariants({
            size,
            variant
          }),
          className
        )}
        aria-disabled={(disabled || loading || false).toString()}
        disabled={disabled || loading}
        ref={ref}
        type={type}
        {...props}
      >
        {children}
        {loading ? '...' : null}
      </Element>
    )
  }
)

Button.displayName = 'Button'

export function CopyButton({
  callback,
  children = 'Copy',
  copiedText = 'Copied!',
  text,
  timeout = 1000,
  variant = 'accent',
  ...props
}: TPolymorphicComponentProps<
  'button',
  TButtonProps & {
    callback?: () => void
    copiedText?: ReactNode
    text: string
    timeout?: number
  }
>) {
  const [copied, setCopied] = useState(false)

  useTimeout(
    () => {
      setCopied(false)
    },
    copied ? timeout ?? null : null
  )

  return (
    <Button
      {...props}
      onClick={() => {
        window.navigator.clipboard
          .writeText(text)
          .then(() => {
            setCopied(true)
            callback?.()
          })
          .catch(() =>
            toast({
              title: `Couldn't copy`,
              variant: 'danger'
            } as Toast)
          )
      }}
      variant={copied ? 'success' : variant}
    >
      {copied ? copiedText : children}
    </Button>
  )
}
