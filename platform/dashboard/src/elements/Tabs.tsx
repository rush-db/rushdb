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
        'bg-background relative flex w-fit gap-1 overflow-auto rounded-md bg-fill2 px-1 py-1',
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
          'relative z-10 inline-flex h-9 shrink-0 items-center gap-3 rounded-md border-b border-transparent px-3 text-sm font-medium whitespace-nowrap outline-hidden focus-visible:ring disabled:cursor-not-allowed disabled:text-content3 [&>svg]:h-[16px] [&>svg]:w-[16px]',
          {
            'text-content2 transition hover:text-content': !active
          },
          className
        )}
        ref={ref}
        {...props}
      >
        {active ?
          <motion.div
            className="absolute start-0 top-0 h-full w-full rounded-md bg-secondary"
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
        'ring-offset-background focus-visible:ring-ring focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-hidden',
        className
      )}
      ref={ref}
      {...props}
    />
  )
)
TabsContent.displayName = Content.displayName
