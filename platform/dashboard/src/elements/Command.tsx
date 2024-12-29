import type { VariantProps } from 'class-variance-authority'

import { Command as CommandPrimitive, useCommandState } from 'cmdk'
import { Search } from 'lucide-react'
import * as React from 'react'

import { cn } from '~/lib/utils'

import { input, inputWrapper } from './Input'
import { menuItem } from './Menu'

export const Command = React.forwardRef<
  React.ElementRef<typeof CommandPrimitive>,
  React.ComponentPropsWithoutRef<typeof CommandPrimitive>
>(({ className, ...props }, ref) => (
  <CommandPrimitive className={cn(className)} ref={ref} {...props} />
))
Command.displayName = CommandPrimitive.displayName

export const CommandInput = React.forwardRef<
  React.ElementRef<typeof CommandPrimitive.Input>,
  Omit<
    React.ComponentPropsWithoutRef<typeof CommandPrimitive.Input>,
    'prefix' | 'size'
  > & {
    prefix?: React.ReactNode
    showSearchIcon?: boolean
    suffix?: React.ReactNode
  } & VariantProps<typeof inputWrapper>
>(
  (
    {
      className,
      placeholder = 'Search...',
      showSearchIcon = true,
      prefix,
      suffix,
      size,
      variant,
      ...props
    },
    ref
  ) => (
    <label
      className={inputWrapper({ size, variant, className })}
      cmdk-input-wrapper=""
    >
      {showSearchIcon && <Search className="shrink-0" />}
      {prefix}
      <CommandPrimitive.Input
        className={input({ size, variant })}
        placeholder={placeholder}
        ref={ref}
        {...props}
      />
      {suffix}
    </label>
  )
)

CommandInput.displayName = CommandPrimitive.Input.displayName

export const CommandList = React.forwardRef<
  React.ElementRef<typeof CommandPrimitive.List>,
  React.ComponentPropsWithoutRef<typeof CommandPrimitive.List>
>(({ className, ...props }, ref) => (
  <CommandPrimitive.List className={cn('', className)} ref={ref} {...props} />
))

CommandList.displayName = CommandPrimitive.List.displayName

export const CommandEmpty = React.forwardRef<
  React.ElementRef<typeof CommandPrimitive.Empty>,
  React.ComponentPropsWithoutRef<typeof CommandPrimitive.Empty>
>((props, ref) => (
  <CommandPrimitive.Empty
    className="py-6 text-center text-sm"
    ref={ref}
    {...props}
  />
))

CommandEmpty.displayName = CommandPrimitive.Empty.displayName

export const CommandGroup = React.forwardRef<
  React.ElementRef<typeof CommandPrimitive.Group>,
  React.ComponentPropsWithoutRef<typeof CommandPrimitive.Group>
>(({ className, ...props }, ref) => (
  <CommandPrimitive.Group
    className={cn(
      'overflow-hidden text-content [&_[cmdk-group-heading]]:sticky [&_[cmdk-group-heading]]:top-0 [&_[cmdk-group-heading]]:border-b [&_[cmdk-group-heading]]:bg-fill [&_[cmdk-group-heading]]:px-3 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-xs [&_[cmdk-group-heading]]:font-medium [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:text-content3',
      className
    )}
    ref={ref}
    {...props}
  />
))

CommandGroup.displayName = CommandPrimitive.Group.displayName

export const CommandSeparator = React.forwardRef<
  React.ElementRef<typeof CommandPrimitive.Separator>,
  React.ComponentPropsWithoutRef<typeof CommandPrimitive.Separator>
>(({ className, ...props }, ref) => (
  <CommandPrimitive.Separator
    className={cn('-mx-1 h-px bg-stroke', className)}
    ref={ref}
    {...props}
  />
))
CommandSeparator.displayName = CommandPrimitive.Separator.displayName

export const CommandItem = React.forwardRef<
  React.ElementRef<typeof CommandPrimitive.Item>,
  React.ComponentPropsWithoutRef<typeof CommandPrimitive.Item> &
    VariantProps<typeof menuItem>
>(({ className, size, variant, ...props }, ref) => (
  <CommandPrimitive.Item
    className={cn(
      menuItem({ size, variant }),
      'flex-row justify-start ',
      className
    )}
    ref={ref}
    {...props}
  />
))

CommandItem.displayName = CommandPrimitive.Item.displayName

export const CommandShortcut = ({
  className,
  ...props
}: React.HTMLAttributes<HTMLSpanElement>) => {
  return (
    <span
      className={cn(
        'ml-auto gap-2 text-xs tracking-widest text-content3',
        className
      )}
      {...props}
    />
  )
}
CommandShortcut.displayName = 'CommandShortcut'

export const SearchItem = React.forwardRef<
  React.ElementRef<typeof CommandItem>,
  React.ComponentPropsWithoutRef<typeof CommandItem> & {
    // exact match with suggested option
    hasMatch?: boolean
  }
>(({ hasMatch, ...props }, ref) => {
  const search = useCommandState((state) => state.search)

  if (hasMatch) {
    return null
  }

  // TODO
  if (!search || search?.length < 1)
    return (
      <CommandItem disabled ref={ref} {...props}>
        <Search />
        {props.placeholder}
      </CommandItem>
    )

  return (
    <CommandItem ref={ref} {...props} value={search}>
      <Search />
      {search}
    </CommandItem>
  )
})
SearchItem.displayName = 'SearchItem'

export { useCommandState } from 'cmdk'
