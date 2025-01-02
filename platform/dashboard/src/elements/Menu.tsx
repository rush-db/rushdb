// @ts-nocheck
import type { DropdownMenuProps, MenuContentProps } from '@radix-ui/react-dropdown-menu'
import type { VariantProps } from 'class-variance-authority'
import type { ComponentPropsWithoutRef, ElementRef, ReactNode } from 'react'

import { Content, Item, Portal, Root, Trigger } from '@radix-ui/react-dropdown-menu'
import { cva } from 'class-variance-authority'
import { ChevronsUpDown } from 'lucide-react'
import { forwardRef } from 'react'

import { cn, composeEventHandlers } from '~/lib/utils'

import { Button } from './Button'
import { $router, openRoute } from '~/lib/router'

// min-w-[clamp(300px,var(--radix-popover-trigger-width),var(--radix-popover-trigger-width))]
export const menuContent = cva(
  'text-menu-content z-50 max-h-[var(--radix-popover-content-available-height)] min-w-[180px] overflow-auto rounded-md border bg-menu shadow-lg animate-in data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2'
)

export const MenuContent = forwardRef<ElementRef<typeof Content>, MenuContentProps>(
  ({ className, sideOffset = 4, ...props }, ref) => (
    <Portal>
      <Content className={menuContent({ className })} ref={ref} sideOffset={sideOffset} {...props} />
    </Portal>
  )
)
MenuContent.displayName = Content.displayName

export const menuItem = cva(
  `flex w-full text-sm cursor-pointer aria-disabled:bg-fill2 aria-disabled:text-content3 aria-disabled:cursor-not-allowed select-none flex-row-reverse items-center justify-between disabled:cursor-not-allowed disabled:text-disabled [&>svg]:shrink-0`,
  {
    variants: {
      size: {
        medium: 'gap-4 px-4 font-medium h-[44px] py-2 [&>svg]:h-[16px] [&>svg]:w-[16px]',
        small: 'gap-3 px-3 font-medium h-[36px] py-1 [&>svg]:h-[16px] [&>svg]:w-[16px]'
      },
      variant: {
        default:
          'hover:bg-menu-hover focus-visible:bg-menu-focus active:bg-menu-hover aria-selected:bg-secondary-hover aria-selected:text-content data-[disabled]:pointer-events-none data-[disabled]:opacity-50',
        danger: 'text-danger hover:bg-danger/30 focus-visible:bg-menu-focus active:bg-menu-hover',
        warning: 'text-warning hover:bg-warning/30 focus-visible:bg-menu-focus active:bg-menu-hover',
        accent: 'text-accent hover:bg-accent/30 focus-visible:bg-menu-focus'
      }
    },
    defaultVariants: {
      variant: 'default',
      size: 'medium'
    }
  }
)

export { Root, Trigger }

export const MenuItem: TPolymorphicComponent<
  ComponentPropsWithoutRef<typeof Item> & {
    dropdown?: boolean
    icon?: ReactNode
    inset?: boolean
  } & VariantProps<typeof menuItem>,
  'div'
> = forwardRef(
  ({ as = 'button', children, className, dropdown, icon, inset, onSelect, variant, ...props }, ref) => {
    const As = as === 'a' ? 'button' : as

    return (
      <Item
        // for dropdowns
        onSelect={(event) => {
          if (dropdown) {
            event.preventDefault()
          }

          onSelect && onSelect(event)
        }}
        asChild
        className={cn(menuItem({ variant, className }), inset && 'pl-8')}
        ref={ref}
        {...props}
      >
        <As
          // for nanostores/router link behavior, too lazy to fix differently now. Without this event.preventDefault() on an anchor tag prevents menu from closing (preventDefault check inside radix-menu)
          {...(as === 'a' ?
            {
              onClick: (event: Event) => {
                $router.open(props.href)
              }
            }
          : {})}
        >
          {icon}
          {children}
        </As>
      </Item>
    )
  }
)
MenuItem.displayName = Item.displayName

export function Menu({
  align = 'end',
  children,
  trigger,
  className,
  open,
  onOpenChange,
  ...contentProps
}: {
  children?: ReactNode
  trigger?: ReactNode
} & MenuContentProps &
  Pick<DropdownMenuProps, 'onOpenChange' | 'open'>) {
  return (
    <Root onOpenChange={onOpenChange} open={open}>
      {trigger ?
        <Trigger asChild>{trigger}</Trigger>
      : null}
      <MenuContent
        className={cn(
          // '[&>[role="menuitem"]+[role="menuitem"]]:border-t',
          className
        )}
        align={align}
        {...contentProps}
      >
        {children}
      </MenuContent>
    </Root>
  )
}

export function MenuTitle({ className, ...props }: TPolymorphicComponentProps<'div'>) {
  return <div className={cn('text-content2 px-4 pt-2 text-[12px] font-semibold', className)} {...props} />
}

export const MenuIcon = ChevronsUpDown

export const MenuButton = forwardRef<
  HTMLButtonElement,
  TInheritableElementProps<'button', { children: ReactNode }>
>(({ children, ...props }, ref) => {
  return (
    <Button ref={ref} size="small" variant="link" {...props}>
      <span className="truncate">{children}</span>
      <MenuIcon />
    </Button>
  )
})

MenuButton.displayName = 'MenuButton'
