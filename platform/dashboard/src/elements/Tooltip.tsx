'use client'

import * as TooltipPrimitive from '@radix-ui/react-tooltip'
import * as React from 'react'

import { cn } from '~/lib/utils'

export const TooltipContent = React.forwardRef<
  React.ElementRef<typeof TooltipPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof TooltipPrimitive.Content>
>(
  (
    { className, side = 'bottom', align = 'start', sideOffset = 4, ...props },
    ref
  ) => (
    // 'z-50 overflow-hidden border bg-menu px-3 py-1.5 text-start text-sm text-menu-contrast shadow-md animate-in fade-in-0 data-[state=closed]:animate-out data-[state=closed]:fade-out-0',
    // 'data-[align=start]:translate-x-[calc(var(--radix-tooltip-trigger-width)-100%)] data-[side=bottom]:rounded-2xl data-[align=end]:rounded-tl-sm data-[align=start]:rounded-tr-sm',
    <TooltipPrimitive.Portal>
      <TooltipPrimitive.Content
        className={cn(
          'z-tooltip flex flex-col overflow-hidden border bg-menu px-2  py-1.5 text-start text-sm text-menu-contrast shadow-md animate-in fade-in-0 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2',
          'data-[side=bottom]:rounded-2xl data-[side=bottom]:rounded-tl-sm',
          'data-[side=left]:translate-y-[var(--radix-tooltip-trigger-height)] data-[side=left]:rounded-2xl data-[side=left]:rounded-tr-sm',
          className
        )}
        align={align}
        ref={ref}
        side={side}
        sideOffset={sideOffset}
        {...props}
      />
    </TooltipPrimitive.Portal>
  )
)
TooltipContent.displayName = TooltipPrimitive.Content.displayName

export const Tooltip = React.forwardRef<
  React.ElementRef<typeof TooltipPrimitive.Trigger>,
  React.ComponentPropsWithoutRef<typeof TooltipPrimitive.TooltipContent> & {
    trigger: React.ReactNode
    open?: boolean
    onOpenChange?: (open: boolean) => void
  }
>(
  (
    {
      trigger,
      alignOffset = 20,
      sideOffset = 10,
      children,
      open,
      onOpenChange,
      ...props
    },
    ref
  ) => {
    return (
      <TooltipPrimitive.TooltipProvider>
        <TooltipPrimitive.Root
          delayDuration={50}
          open={open}
          onOpenChange={onOpenChange}
        >
          <TooltipPrimitive.Trigger asChild ref={ref}>
            {trigger}
          </TooltipPrimitive.Trigger>

          <TooltipContent
            alignOffset={alignOffset}
            sideOffset={sideOffset}
            {...props}
          >
            {children}
          </TooltipContent>
        </TooltipPrimitive.Root>
      </TooltipPrimitive.TooltipProvider>
    )
  }
)
Tooltip.displayName = 'Tooltip'
