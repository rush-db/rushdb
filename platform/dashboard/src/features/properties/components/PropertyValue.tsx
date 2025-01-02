// import type { FocusEvent, KeyboardEvent, ReactEventHandler } from 'react'

// import { Edit } from 'lucide-react'
// import { useRef } from 'react'

// import {
//   HoverCard,
//   HoverCardContent,
//   HoverCardTrigger
// } from '~/elements/HoverCard'
import type { PropertyWithValue } from '@rushdb/javascript-sdk'

// import { IconButton, IconCopyButton } from '~/elements/IconButton'
import { cn } from '~/lib/utils'

import { formatPropertyValue } from '../utils'

export function PropertyValue({
  className,
  value,
  type,
  ...props
}: TPolymorphicComponentProps<'span', Pick<PropertyWithValue, 'type' | 'value'>>) {
  const formatted = formatPropertyValue({ value, type })

  return (
    <span
      title={formatted}
      {...props}
      className={cn(
        'font-bold',
        {
          'font-mono': type === 'number'
        },
        className
      )}
    >
      {formatted}
    </span>
  )
}
