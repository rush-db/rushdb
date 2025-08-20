import { Check, ChevronsUpDown } from 'lucide-react'
import * as React from 'react'

import { useControllableState } from '~/hooks/useControllableState'
import { cn, composeEventHandlers } from '~/lib/utils'

import { Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList } from './Command'
import { DisclosureContext, useDisclosure, useDisclosureContext } from './Disclosure'
import { Popover, PopoverContent, PopoverTrigger } from './Popover'

type SelectValue = string

type Props = Pick<React.ComponentPropsWithoutRef<typeof Popover>, 'onOpenChange' | 'open'> & {
  asChild?: boolean
  children?: React.ReactNode
  onChange?: (value: SelectValue) => void
  trigger: React.ReactNode
  value?: SelectValue
}

export function SearchSelect({
  onOpenChange: onOpenChangeProp,
  open: openProp,
  trigger,
  asChild = true,
  // value: valueProp,
  // onChange: onChangeProp,
  children
}: Props) {
  // const [value, setValue] = useControllableState({
  //   value: valueProp,
  //   onChange: onChangeProp
  // })
  const ctx = useDisclosure({
    open: openProp,
    onOpenChange: onOpenChangeProp
  })

  return (
    <DisclosureContext.Provider value={ctx}>
      <Popover onOpenChange={ctx.setOpen} open={ctx.isOpen}>
        <PopoverTrigger asChild={asChild}>{trigger}</PopoverTrigger>

        <PopoverContent align="start">
          <Command>
            <CommandInput className="rounded-none" placeholder="Search..." size="small" variant="ghost" />
            <CommandList>
              <CommandEmpty>Nothing found.</CommandEmpty>
              <CommandGroup>{children}</CommandGroup>
            </CommandList>
          </Command>
        </PopoverContent>
      </Popover>
    </DisclosureContext.Provider>
  )
}

export const SelectItem = React.forwardRef<
  React.ElementRef<typeof CommandItem>,
  React.ComponentPropsWithoutRef<typeof CommandItem> & {
    closeOnSelect?: boolean
  }
>(({ onSelect, closeOnSelect = true, ...props }, ref) => {
  const ctx = useDisclosureContext()

  return (
    <CommandItem
      size="small"
      {...props}
      onSelect={composeEventHandlers(
        onSelect,
        closeOnSelect ?
          () => {
            ctx.close()
          }
        : undefined
      )}
      className={cn('h-9', props.className)}
      ref={ref}
    />
  )
})

SelectItem.displayName = 'SelectItem'
