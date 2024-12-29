import type { DialogPortalProps, DialogProps } from '@radix-ui/react-dialog'
import type {
  ComponentPropsWithoutRef,
  ElementRef,
  HTMLAttributes,
  PropsWithChildren,
  ReactNode
} from 'react'

import {
  Close,
  Content,
  Overlay,
  Portal,
  Root,
  Title,
  Trigger
} from '@radix-ui/react-dialog'
import { X } from 'lucide-react'
import { forwardRef } from 'react'

import { cn } from '~/lib/utils'

import { IconButton } from './IconButton'
import { Spinner } from './Spinner'

const DialogPortal = ({ children, ...props }: DialogPortalProps) => (
  <Portal {...props}>
    <div className="fixed inset-0 z-50 flex items-start justify-center sm:items-center">
      {children}
    </div>
  </Portal>
)
DialogPortal.displayName = Portal.displayName

const DialogOverlay = forwardRef<
  ElementRef<typeof Overlay>,
  ComponentPropsWithoutRef<typeof Overlay>
>(({ className, ...props }, ref) => (
  <Overlay
    className={cn(
      'fixed inset-0 z-50 bg-fill2/60 transition-all duration-100 data-[state=closed]:animate-out data-[state=closed]:fade-out data-[state=open]:fade-in',
      className
    )}
    ref={ref}
    {...props}
  />
))
DialogOverlay.displayName = 'DialogOverlay'

export const DialogFooter = ({
  className,
  ...props
}: HTMLAttributes<HTMLDivElement>) => (
  <footer
    className={cn(
      'sticky bottom-0 flex flex-col gap-2 bg-fill2 sm:flex-row sm:[&>*]:flex-1',
      className
    )}
    {...props}
  />
)

const DialogBody = forwardRef<
  ElementRef<typeof Content>,
  ComponentPropsWithoutRef<typeof Content>
>(({ children, className, ...props }, ref) => (
  <DialogPortal>
    <DialogOverlay />
    <Content
      className={cn(
        'fixed z-50 flex h-fit max-h-screen w-full flex-col overflow-auto rounded-b-xl border bg-fill p-6 px-6 shadow-2xl animate-in data-[state=open]:fade-in-90 data-[state=open]:slide-in-from-bottom-10 sm:max-w-lg sm:rounded-xl sm:zoom-in-90 data-[state=open]:sm:slide-in-from-bottom-0',
        className
      )}
      ref={ref}
      {...props}
    >
      {children}
      <Close
        asChild
        className="ring-offset-background absolute end-4 top-4 opacity-70  transition-opacity hover:opacity-100 focus:outline-none focus:ring-2 focus:ring-offset-2 focus-visible:ring disabled:pointer-events-none"
      >
        <IconButton aria-label="close" variant="ghost">
          <X />
        </IconButton>
      </Close>
    </Content>
  </DialogPortal>
))
DialogBody.displayName = Content.displayName

export const DialogTitle = forwardRef<
  ElementRef<typeof Title>,
  ComponentPropsWithoutRef<typeof Title>
>(({ className, ...props }, ref) => (
  <Title
    className={cn('flex gap-3 text-lg font-bold tracking-tight', className)}
    ref={ref}
    {...props}
  />
))
DialogTitle.displayName = Title.displayName

export function DialogLoadingOverlay() {
  return (
    <div className="absolute left-0 top-0 grid h-full w-full place-items-center rounded-lg bg-fill2/70">
      <Spinner />
    </div>
  )
}

export type TDialogProps = PropsWithChildren<{
  loading?: boolean
  trigger?: ReactNode
}> &
  ComponentPropsWithoutRef<typeof Content> &
  Pick<DialogProps, 'onOpenChange' | 'open'>

export function Dialog({
  children,
  onOpenChange,
  open,
  trigger,
  loading,
  ...props
}: TDialogProps) {
  return (
    <Root onOpenChange={onOpenChange} open={open}>
      <Trigger asChild>{trigger}</Trigger>
      <DialogBody {...props}>
        {children}
        {loading && <DialogLoadingOverlay />}
      </DialogBody>
    </Root>
  )
}

export { Close }
