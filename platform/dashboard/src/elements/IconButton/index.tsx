import { type VariantProps, cva } from 'class-variance-authority'
import { Copy, CopyCheck } from 'lucide-react'
import { forwardRef, useState } from 'react'

import { useHotkeys } from '~/hooks/useHotkeys'
import { useTimeout } from '~/hooks/useTimeout'
import { cn, composeEventHandlers, copyToClipboard } from '~/lib/utils'

import type { Toast } from '../Toast'

import { buttonVariants } from '../Button'
import { Kbd } from '../Kbd'
import { toast } from '../Toast'
import { Tooltip } from '../Tooltip'

export const iconButton = cva<{ size: Record<string, string> }>('p-0 min-w-0', {
  variants: {
    size: {
      large: 'w-[60px]',
      medium: 'w-[44px]',
      small: 'w-[36px]',
      xsmall: 'w-[28px]',
      xxsmall: 'w-[20px]'
    }
  },
  defaultVariants: {
    size: 'medium'
  }
})

type IconButtonProps = {
  ['aria-label']: string
} & VariantProps<typeof buttonVariants>

// @ts-ignore
export const IconButton: TPolymorphicComponent<IconButtonProps, 'button'> = forwardRef(
  ({ className, as = 'button', type = 'button', size, variant, ...props }, ref) => {
    const Element = as

    return (
      <Element
        className={cn('shrink-none', buttonVariants({ size, variant, className }), iconButton({ size }))}
        ref={ref}
        title={props['aria-label']}
        type={type}
        {...props}
      />
    )
  }
)

IconButton.displayName = 'IconButton'

export function IconCopyButton({
  text,
  timeout = 1000,
  variant = 'accent',
  onClick,
  ...props
}: TPolymorphicComponentProps<
  'button',
  Omit<IconButtonProps, 'aria-label'> & {
    text: string
    timeout?: number
  }
>) {
  const [open, setOpen] = useState(false)
  const [copied, setCopied] = useState(false)

  const copy = () => {
    if (!open) {
      return
    }

    copyToClipboard(text, {
      callback: () => setCopied(true)
    })
  }

  useTimeout(
    () => {
      setCopied(false)
    },
    copied ? (timeout ?? null) : null
  )

  useHotkeys({
    'Meta+C': copy,
    'Ctrl+C': copy
  })

  return (
    <Tooltip
      open={open}
      onOpenChange={setOpen}
      trigger={
        <IconButton
          {...props}
          aria-label="Copy"
          onClick={composeEventHandlers(copy, onClick)}
          title={copied ? 'Copied' : 'Copy'}
          variant={copied ? 'success' : variant}
        >
          {copied ?
            <CopyCheck />
          : <Copy />}
        </IconButton>
      }
    >
      <Kbd code={['Meta', 'C']}>Copy</Kbd>
    </Tooltip>
  )
}
