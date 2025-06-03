import { useStore } from '@nanostores/react'
import { type ReactNode } from 'react'

import { Tab, Tabs, TabsList } from '~/elements/Tabs'
import { $router } from '~/lib/router'

export function PageTab({
  href,
  icon,
  label,
  dataTour
}: {
  href: string
  icon?: ReactNode
  label?: ReactNode
  dataTour?: string
}) {
  return (
    <Tab layoutId="PAGE_TAB" value={href} data-tour={dataTour}>
      {icon}
      {label}
    </Tab>
  )
}

export function PageTabs({ className, ...props }: TPolymorphicComponentProps<'nav'>) {
  const page = useStore($router)

  return (
    <Tabs
      className={className}
      onValueChange={(path) => $router.open(path + location.search)}
      value={page?.path}
    >
      <TabsList className="w-full overflow-auto rounded-none border-b px-2 sm:px-5">
        {props.children}
      </TabsList>
    </Tabs>
  )
}
