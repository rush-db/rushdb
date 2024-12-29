import * as React from 'react'

import { useClickOutside } from '~/hooks/useClickOutside'
import { useFocusOutside } from '~/hooks/useFocusOutside'
import { cn, composeEventHandlers } from '~/lib/utils'

import type { DisclosureOptions } from './Disclosure'

import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList
} from './Command'
import {
  DisclosureContext,
  useDisclosure,
  useDisclosureContext
} from './Disclosure'
import { Popover, PopoverAnchor, PopoverContent } from './Popover'

export const Combobox = React.forwardRef<
  React.ElementRef<typeof Command>,
  React.ComponentPropsWithoutRef<typeof Command> & DisclosureOptions
>(
  (
    { className, loop = true, onKeyDown, open, onOpenChange, ...props },
    ref
  ) => {
    const ctx = useDisclosure({ open, onOpenChange })

    const { onPointerDown } = useClickOutside(ctx.close)
    const { onFocus } = useFocusOutside(ctx.close)

    return (
      <DisclosureContext.Provider value={ctx}>
        <Popover modal={false} open={ctx.isOpen}>
          <Command
            className={cn('w-full', className)}
            ref={ref}
            {...props}
            onKeyDown={composeEventHandlers(onKeyDown, (event) => {
              if (event.key === 'Escape') {
                if (ctx.isOpen) {
                  event.stopPropagation()
                  ctx.close()
                }
              } else if (event.key !== 'Tab') {
                ctx.open()
              }
            })}
            onPointerDown={composeEventHandlers(
              props.onPointerDown,
              onPointerDown
            )}
            loop={loop}
            onFocus={composeEventHandlers(props.onFocus, onFocus)}
          />
        </Popover>
      </DisclosureContext.Provider>
    )
  }
)
Combobox.displayName = 'Combobox'

export const ComboboxInput = React.forwardRef<
  React.ElementRef<typeof CommandInput>,
  React.ComponentPropsWithoutRef<typeof CommandInput>
>(({ className, placeholder = 'Search...', ...props }, ref) => {
  const { open } = useDisclosureContext()
  return (
    <PopoverAnchor className="w-full">
      <CommandInput
        className={className}
        onClick={open}
        onFocus={open}
        placeholder={placeholder}
        ref={ref}
        {...props}
      />
    </PopoverAnchor>
  )
})
ComboboxInput.displayName = 'ComboboxInput'

export const ComboboxPopover = React.forwardRef<
  React.ElementRef<typeof PopoverContent>,
  React.ComponentPropsWithoutRef<typeof PopoverContent>
>(({ className, ...props }, ref) => (
  <PopoverContent
    className={cn(
      'max-h-[var(--radix-popover-content-available-height)] w-[var(--radix-popper-anchor-width)] overflow-auto p-0',
      className
    )}
    onOpenAutoFocus={(event) => {
      event.preventDefault()
    }}
    align="start"
    asChild
    portalled={false} // iDk
    ref={ref}
    side="bottom"
    {...props}
  />
))
ComboboxPopover.displayName = 'ComboboxPopover'

export const ComboboxList = React.forwardRef<
  React.ElementRef<typeof CommandList>,
  React.ComponentPropsWithoutRef<typeof CommandList>
>(({ className, ...props }, ref) => (
  <CommandList className={cn(className)} ref={ref} {...props} />
))
ComboboxList.displayName = 'ComboboxPopover'

export const ComboboxEmpty = CommandEmpty

export const ComboboxGroup = CommandGroup

export const ComboboxItem = CommandItem

export const ComboboxTitle = React.forwardRef<HTMLDivElement, TPropsOf<'div'>>(
  ({ className, ...props }, ref) => (
    <div
      className={cn(
        'bg-fill px-3 py-1.5 text-xs font-medium uppercase text-content3',
        className
      )}
      {...props}
      ref={ref}
    />
  )
)
// 'overflow-hidden py-1 text-content [&_[cmdk-group-heading]]:sticky [&_[cmdk-group-heading]]:top-0 [&_[cmdk-group-heading]]:border-b [&_[cmdk-group-heading]]:bg-fill [&_[cmdk-group-heading]]:px-3 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-xs [&_[cmdk-group-heading]]:font-medium [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:text-content3',
ComboboxTitle.displayName = 'ComboboxTitle'

export { SearchItem } from './Command'
