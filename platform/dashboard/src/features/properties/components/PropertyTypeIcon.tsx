import type { PropertyType } from '@rushdb/javascript-sdk'

import { Calendar, CircleSlash2, Hash, ToggleRight, Type } from 'lucide-react'

export function PropertyTypeIcon({ size, type }: { size?: number | string; type: PropertyType }) {
  switch (type) {
    case 'boolean':
      return <ToggleRight size={size} />
    case 'number':
      return <Hash size={size} />
    case 'string':
      return <Type size={size} />
    case 'datetime':
      return <Calendar size={size} />
    case 'null':
      return <CircleSlash2 size={size} />
    default:
      console.warn(`No icon match for property type ${type}`)
      return null
  }
}
