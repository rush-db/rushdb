import type { DialogPortalProps, DialogProps } from '@radix-ui/react-dialog'
import type { VariantProps } from 'class-variance-authority'
import type {
  ComponentPropsWithoutRef,
  ElementRef,
  HTMLAttributes,
  ReactNode
} from 'react'

import { Content, Overlay, Portal, Root, Trigger } from '@radix-ui/react-dialog'
import { cva } from 'class-variance-authority'
import { forwardRef } from 'react'

import { cn } from '~/lib/utils'

const portalVariants = cva<{
  position: Record<string, string>
}>('fixed inset-0 z-50 flex', {
  defaultVariants: { position: 'right' },
  variants: {
    position: {
      bottom: 'items-end',
      left: 'justify-start',
      right: 'justify-end',
      top: 'items-start'
    }
  }
})

interface SheetPortalProps
  extends DialogPortalProps,
    VariantProps<typeof portalVariants> {}

const SheetPortal = ({ children, position, ...props }: SheetPortalProps) => (
  <Portal {...props}>
    <div className={portalVariants({ position })} data-portal="true">
      {children}
    </div>
  </Portal>
)
SheetPortal.displayName = Portal.displayName

const SheetOverlay = forwardRef<
  ElementRef<typeof Overlay>,
  ComponentPropsWithoutRef<typeof Overlay>
>(({ children, className, ...props }, ref) => (
  <Overlay
    className={cn(
      'fixed inset-0 z-50 bg-fill2/60 transition-all duration-100 data-[state=closed]:animate-out data-[state=closed]:fade-out data-[state=open]:fade-in',
      className
    )}
    {...props}
    ref={ref}
  />
))
SheetOverlay.displayName = Overlay.displayName

const sheetVariants = cva<{
  position: Record<string, string>
  size: Record<string, string>
}>('fixed overflow-auto z-50 scale-100 gap-4 bg-fill opacity-100 shadow-lg', {
  compoundVariants: [
    {
      class: 'max-h-screen',
      position: ['top', 'bottom'],
      size: 'content'
    },
    {
      class: 'h-1/3',
      position: ['top', 'bottom'],
      size: 'default'
    },
    {
      class: 'h-1/4',
      position: ['top', 'bottom'],
      size: 'sm'
    },
    {
      class: 'h-1/2',
      position: ['top', 'bottom'],
      size: 'lg'
    },
    {
      class: 'h-5/6',
      position: ['top', 'bottom'],
      size: 'xl'
    },
    {
      class: 'h-screen',
      position: ['top', 'bottom'],
      size: 'full'
    },
    {
      class: 'max-w-screen',
      position: ['right', 'left'],
      size: 'content'
    },
    {
      class: 'w-1/3',
      position: ['right', 'left'],
      size: 'default'
    },
    {
      class: 'w-1/4',
      position: ['right', 'left'],
      size: 'sm'
    },
    {
      class: 'w-full max-w-lg',
      position: ['right', 'left'],
      size: 'lg'
    },
    {
      class: 'w-5/6',
      position: ['right', 'left'],
      size: 'xl'
    },
    {
      class: 'w-screen',
      position: ['right', 'left'],
      size: 'full'
    }
  ],
  defaultVariants: {
    position: 'right',
    size: 'default'
  },
  variants: {
    position: {
      bottom: 'animate-in slide-in-from-bottom w-full duration-150',
      left: 'animate-in slide-in-from-left h-full duration-150',
      right: 'animate-in slide-in-from-right h-full duration-150 border-l',
      top: 'animate-in slide-in-from-top w-full duration-150'
    },
    size: {
      content: '',
      default: '',
      full: '',
      lg: '',
      sm: '',
      xl: ''
    }
  }
})

export interface DialogContentProps
  extends ComponentPropsWithoutRef<typeof Content>,
    VariantProps<typeof sheetVariants> {}

const SheetContent = forwardRef<ElementRef<typeof Content>, DialogContentProps>(
  ({ children, className, position, size, ...props }, ref) => (
    <SheetPortal position={position}>
      <SheetOverlay />
      <Content
        className={cn(sheetVariants({ position, size }), className)}
        ref={ref}
        {...props}
      >
        {children}
      </Content>
    </SheetPortal>
  )
)
SheetContent.displayName = Content.displayName

const SheetHeader = ({
  className,
  ...props
}: HTMLAttributes<HTMLDivElement>) => (
  <div
    className={cn(
      'flex flex-col space-y-2 text-center sm:text-left',
      className
    )}
    {...props}
  />
)
SheetHeader.displayName = 'SheetHeader'

const SheetFooter = ({
  className,
  ...props
}: HTMLAttributes<HTMLDivElement>) => (
  <div
    className={cn(
      'flex flex-col-reverse sm:flex-row sm:justify-end sm:space-x-2',
      className
    )}
    {...props}
  />
)
SheetFooter.displayName = 'SheetFooter'

export type TSheetProps = {
  children?: ReactNode
  trigger?: ReactNode
} & DialogProps &
  VariantProps<typeof sheetVariants>

export function Sheet({
  children,
  position = 'right',
  size = 'lg',
  trigger,
  ...props
}: TSheetProps) {
  return (
    <Root {...props}>
      {trigger && <Trigger asChild>{trigger}</Trigger>}
      <SheetContent position={position} size={size}>
        {children}
      </SheetContent>
    </Root>
  )
}

export { Close } from '@radix-ui/react-dialog'
