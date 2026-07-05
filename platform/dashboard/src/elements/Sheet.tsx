import type { DialogProps } from '@radix-ui/react-dialog'
import type { ReactNode } from 'react'

import { Content, Root, Trigger } from '@radix-ui/react-dialog'

import { cn } from '~/lib/utils'

export type TSheetProps = {
  children?: ReactNode
  className?: string
  trigger?: ReactNode
} & DialogProps

/**
 * Docked side panel. Renders in-flow (no portal, no overlay) so it takes up
 * layout width and shrinks its flex siblings instead of overlapping them.
 * Mount it as a direct child of a horizontal flex container.
 */
export function Sheet({ children, className, trigger, ...props }: TSheetProps) {
  return (
    <Root modal={false} {...props}>
      {trigger && <Trigger asChild>{trigger}</Trigger>}
      <Content
        className={cn(
          'flex w-full max-w-lg shrink-0 animate-in flex-col overflow-y-auto border-l bg-fill duration-150 slide-in-from-right',
          className
        )}
        aria-describedby={undefined}
        onCloseAutoFocus={(event) => event.preventDefault()}
        onInteractOutside={(event) => event.preventDefault()}
        onOpenAutoFocus={(event) => event.preventDefault()}
      >
        {children}
      </Content>
    </Root>
  )
}

export { Close } from '@radix-ui/react-dialog'
