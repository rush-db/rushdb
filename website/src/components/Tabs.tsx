import type { ComponentPropsWithoutRef, ElementRef } from 'react'

import { Content, List, Trigger } from '@radix-ui/react-tabs'
import { forwardRef } from 'react'

import cn from 'classnames'

export { Root as Tabs } from '@radix-ui/react-tabs'

import { motion } from 'framer-motion'

export const TabsList = forwardRef<ElementRef<typeof List>, ComponentPropsWithoutRef<typeof List>>(
  ({ className, ...props }, ref) => (
    <List
      className={cn(
        'bg-fill2 relative inline-flex gap-1 overflow-auto rounded-lg px-2 py-2 sm:w-full',
        className
      )}
      ref={ref}
      {...props}
    />
  )
)
TabsList.displayName = List.displayName

// @ts-ignore
const TabInner: TPolymorphicComponent<{ layoutId?: string }, 'button'> = forwardRef(
  ({ children, className, layoutId = 'tabs', as = 'button', ...props }, ref) => {
    const active = 'data-state' in props && props['data-state'] === 'active'

    const Component = as

    return (
      <Component
        className={cn(
          'relative z-10 inline-flex h-9 min-w-48 shrink-0 items-center justify-center gap-3 whitespace-nowrap rounded-md px-3 text-center text-sm font-medium outline-none focus-visible:ring sm:min-w-0 sm:flex-1 [&>svg]:h-[16px] [&>svg]:w-[16px]',
          {
            'text-content2 hover:text-content transition': !active
          },
          className
        )}
        ref={ref}
        {...props}
      >
        {active ?
          <motion.div
            className="bg-secondary absolute start-0 top-0 h-full w-full rounded-md"
            layoutId={layoutId}
          />
        : null}
        {children}
      </Component>
    )
  }
)
TabInner.displayName = 'TabInner'

// @ts-ignore
export const Tab: TPolymorphicComponent<
  ComponentPropsWithoutRef<typeof Trigger> & { layoutId?: string },
  'button'
> = forwardRef(({ children, as, ...props }, ref) => {
  return (
    <>
      {/*@ts-ignore*/}
      <Trigger asChild ref={ref} {...props}>
        <TabInner as={as}>{children}</TabInner>
      </Trigger>
    </>
  )
})
Tab.displayName = Trigger.displayName

export const TabsContent = forwardRef<ElementRef<typeof Content>, ComponentPropsWithoutRef<typeof Content>>(
  ({ className, ...props }, ref) => (
    <Content
      className={cn(
        'ring-offset-background focus-visible:ring-ring focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2',
        className
      )}
      ref={ref}
      {...props}
    />
  )
)
TabsContent.displayName = Content.displayName
