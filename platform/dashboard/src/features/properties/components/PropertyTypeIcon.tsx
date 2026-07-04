import type { PropertyType } from '@rushdb/javascript-sdk'
import type { LucideIcon } from 'lucide-react'
import type { ReactNode } from 'react'

import { Calendar, Hash, ToggleRight, Type } from 'lucide-react'

import { cn } from '~/lib/utils'

// Badge colors are theme-shared and each carries a generated `contrast`
// counterpart, so the icon stays legible in both light and dark themes.
const typeIcons: Partial<Record<PropertyType, { className: string; icon: LucideIcon }>> = {
  boolean: { className: 'bg-badge-orange text-badge-orange-contrast', icon: ToggleRight },
  datetime: { className: 'bg-badge-green text-badge-green-contrast', icon: Calendar },
  number: { className: 'bg-badge-blue text-badge-blue-contrast', icon: Hash },
  string: { className: 'bg-badge-yellow text-badge-yellow-contrast', icon: Type }
}

const toNumericSize = (size: number | string | undefined): number => {
  if (typeof size === 'number') return size
  const parsed = Number.parseInt(size ?? '', 10)
  return Number.isNaN(parsed) ? 16 : parsed
}

/** Rounded color square hosting a property icon; shared by PropertyName for the primary-key icon. */
export function PropertyIconSquare({
  children,
  className,
  size
}: {
  children: ReactNode
  className?: string
  size?: number | string
}) {
  const squareSize = toNumericSize(size)

  return (
    <span
      className={cn('inline-flex shrink-0 items-center justify-center rounded', className)}
      style={{ height: squareSize, width: squareSize }}
    >
      {children}
    </span>
  )
}

export function PropertyTypeIcon({ size, type }: { size?: number | string; type: PropertyType }) {
  const match = typeIcons[type]

  if (!match) {
    console.warn(`No icon match for property type ${type}`)
    return null
  }

  const { className, icon: Icon } = match

  return (
    <PropertyIconSquare className={className} size={size}>
      <Icon size={Math.round(toNumericSize(size) * 0.7)} />
    </PropertyIconSquare>
  )
}
