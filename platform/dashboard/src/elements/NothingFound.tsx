import type { ComponentProps } from 'react'

import { SearchX } from 'lucide-react'

import { Banner } from './Banner'

export function NothingFound({
  className,
  title = 'Nothing found...',
  action
}: TPolymorphicComponentProps<'div', ComponentProps<typeof Banner>>) {
  return (
    <Banner
      action={action}
      className={className}
      image={<SearchX size={64} />}
      title={title}
    />
  )
}
