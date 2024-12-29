'use client'

import * as PopoverPrimitive from '@radix-ui/react-popover'
import * as React from 'react'

import { menuContent } from './Menu'

const Popover = PopoverPrimitive.Root

const PopoverTrigger = PopoverPrimitive.Trigger

export const PopoverAnchor = React.forwardRef<
  React.ElementRef<typeof PopoverPrimitive.Anchor>,
  React.ComponentPropsWithoutRef<typeof PopoverPrimitive.Anchor>
>((props, ref) => {
  return <PopoverPrimitive.Anchor {...props} ref={ref} />
})
PopoverAnchor.displayName = 'PopoverAnchor'

const PopoverContent = React.forwardRef<
  React.ElementRef<typeof PopoverPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof PopoverPrimitive.Content> & {
    portalled?: boolean
  }
>(
  (
    { className, align = 'start', sideOffset = 4, portalled = true, ...props },
    ref
  ) => {
    const Wrapper = portalled ? PopoverPrimitive.Portal : React.Fragment
    return (
      <Wrapper>
        <PopoverPrimitive.Content
          align={align}
          className={menuContent({ className })}
          ref={ref}
          sideOffset={sideOffset}
          {...props}
        />
      </Wrapper>
    )
  }
)
PopoverContent.displayName = PopoverPrimitive.Content.displayName

export { Popover, PopoverContent, PopoverTrigger }
