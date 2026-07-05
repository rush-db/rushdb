import { useStore } from '@nanostores/react'
import { Children, cloneElement, isValidElement, type ReactElement, type ReactNode } from 'react'

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

export function PageTabSectionHeader({ title, collapsed }: { title: string; collapsed?: boolean }) {
  if (collapsed) {
    return <div aria-hidden className="mx-1 my-2 h-px bg-stroke" />
  }

  return (
    <div className="px-2 pt-3 pb-1 text-2xs font-medium tracking-wide text-content3 uppercase">{title}</div>
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
          Children.map(props.children, (child) => {
            if (!isValidElement(child)) {
              return child
            }

            if (child.type === PageTab) {
              return cloneElement(child as ReactElement<{ className?: string; hideLabel?: boolean }>, {
                className: cn(
                  collapsed ? 'w-full justify-center px-0' : 'w-full justify-start',
                  (child.props as { className?: string }).className
                ),
                hideLabel: collapsed
              })
            }

            if (child.type === PageTabSectionHeader) {
              return cloneElement(child as ReactElement<{ collapsed?: boolean }>, { collapsed })
            }

            return child
          })
        : props.children}
      </TabsList>
    </Tabs>
  )
}
