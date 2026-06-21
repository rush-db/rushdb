import { useStore } from '@nanostores/react'
import { Children, cloneElement, isValidElement, type ReactNode } from 'react'

import { Tab, Tabs, TabsList } from '~/elements/Tabs'
import { $router } from '~/lib/router'
import { cn } from '~/lib/utils'

export function PageTab({
  className,
  href,
  icon,
  label,
  hideLabel,
  dataTour,
  onClick,
  primary
}: {
  className?: string
  href: string
  icon?: ReactNode
  label?: ReactNode
  hideLabel?: boolean
  dataTour?: string
  onClick?: () => void
  primary?: boolean
}) {
  return (
    <Tab
      className={cn(
        primary && 'bg-primary text-primary-contrast hover:bg-primary-hover [&_svg]:text-primary-contrast',
        className
      )}
      layoutId="PAGE_TAB"
      value={href}
      data-tour={dataTour}
      onClick={onClick}
    >
      {icon}
      {!hideLabel && label}
    </Tab>
  )
}

export function PageTabs({
  className,
  collapsed = false,
  variant = 'top',
  ...props
}: TPolymorphicComponentProps<'nav'> & { collapsed?: boolean; variant?: 'top' | 'sidebar' }) {
  const page = useStore($router)
  const sidebar = variant === 'sidebar'

  return (
    <Tabs
      className={className}
      onValueChange={(path) => $router.open(path + location.search)}
      value={page?.path}
    >
      <TabsList
        className={cn(
          sidebar ?
            'w-full flex-col items-stretch gap-1 overflow-visible rounded-none bg-transparent p-0'
          : 'w-full overflow-auto rounded-none border-b px-2 sm:px-5'
        )}
      >
        {sidebar ?
          Children.map(props.children, (child) =>
            isValidElement<{ className?: string; hideLabel?: boolean }>(child) && child.type === PageTab ?
              cloneElement(child, {
                className: cn(
                  collapsed ? 'w-full justify-center px-0' : 'w-full justify-start',
                  child.props.className
                ),
                hideLabel: collapsed
              })
            : child
          )
        : props.children}
      </TabsList>
    </Tabs>
  )
}
