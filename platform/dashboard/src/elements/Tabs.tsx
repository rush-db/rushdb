// @ts-nocheck
import type { ComponentPropsWithoutRef, ElementRef } from 'react'

import { Content, List, Root, Trigger } from '@radix-ui/react-tabs'
import { createContext, forwardRef, useContext, useId } from 'react'

import { cn } from '~/lib/utils'

import { motion } from 'framer-motion'

const TabsLayoutContext = createContext<string | undefined>(undefined)

export const Tabs = forwardRef<
  ElementRef<typeof Root>,
  ComponentPropsWithoutRef<typeof Root> & { layoutId?: string }
>(({ layoutId, ...props }, ref) => {
  const generatedLayoutId = useId()

  return (
    <TabsLayoutContext.Provider value={layoutId ?? `tabs-${generatedLayoutId}`}>
      <Root ref={ref} {...props} />
    </TabsLayoutContext.Provider>
  )
})
Tabs.displayName = Root.displayName

export const TabsList = forwardRef<ElementRef<typeof List>, ComponentPropsWithoutRef<typeof List>>(
  ({ className, ...props }, ref) => (
    <List
      className={cn(
        'bg-background bg-fill2 relative flex w-fit gap-1 overflow-auto rounded-md px-1 py-1',
        className
      )}
      ref={ref}
      {...props}
    />
  )
)
TabsList.displayName = List.displayName

const TabInner: TPolymorphicComponent<{ layoutId?: string }, 'button'> = forwardRef(
  ({ children, className, layoutId, as = 'button', ...props }, ref) => {
    const active = 'data-state' in props && props['data-state'] === 'active'
    const contextLayoutId = useContext(TabsLayoutContext)

    const Component = as

    return (
      <Component
        className={cn(
          'disabled:text-content3 relative z-10 inline-flex h-9 shrink-0 items-center gap-3 whitespace-nowrap rounded border-b border-transparent px-3 text-sm font-medium outline-none focus-visible:ring disabled:cursor-not-allowed [&>svg]:h-[16px] [&>svg]:w-[16px]',
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
            className="bg-secondary absolute start-0 top-0 h-full w-full rounded"
            layoutId={layoutId ?? contextLayoutId}
          />
        : null}
        {children}
      </Component>
    )
  }
)
TabInner.displayName = 'TabInner'

export const Tab: TPolymorphicComponent<
  ComponentPropsWithoutRef<typeof Trigger> & { layoutId?: string },
  'button'
> = forwardRef(({ children, as, layoutId, ...props }, ref) => {
  return (
    <Trigger asChild ref={ref} {...props}>
      <TabInner as={as} layoutId={layoutId}>
        {children}
      </TabInner>
    </Trigger>
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
