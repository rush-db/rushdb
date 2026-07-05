import type { PropertyType } from '@rushdb/javascript-sdk'
import type { LucideIcon } from 'lucide-react'
import type { ReactNode } from 'react'

import { Calendar, Hash, ToggleRight, Type } from 'lucide-react'

import { cn } from '~/lib/utils'

// Dimmed same-hue background with the bright badge color for the glyph —
// the tinted-badge pattern also used by IndexesList and saved-queries.
// Badge tokens are shared between themes, so on light surfaces the glyph is
// mixed toward black (yellow most — it's the lightest hue) for contrast.
const typeIcons: Partial<Record<PropertyType, { className: string; icon: LucideIcon }>> = {
  boolean: {
    className:
      'bg-badge-orange/15 text-badge-orange light:bg-badge-orange/25 light:text-[color-mix(in_oklab,hsl(var(--badge-orange))_75%,black)]',
    icon: ToggleRight
  },
  datetime: {
    className:
      'bg-badge-green/15 text-badge-green light:bg-badge-green/25 light:text-[color-mix(in_oklab,hsl(var(--badge-green))_75%,black)]',
    icon: Calendar
  },
  number: {
    className:
      'bg-badge-blue/15 text-badge-blue light:bg-badge-blue/25 light:text-[color-mix(in_oklab,hsl(var(--badge-blue))_75%,black)]',
    icon: Hash
  },
  string: {
    className:
      'bg-badge-yellow/15 text-badge-yellow light:bg-badge-yellow/25 light:text-[color-mix(in_oklab,hsl(var(--badge-yellow))_60%,black)]',
    icon: Type
  }
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
      className={cn(
        'inline-flex shrink-0 items-center justify-center rounded-md leading-none [&>svg]:block',
        className
      )}
      style={{ height: squareSize, width: squareSize }}
    >
      {children}
    </span>
  )
}

// Even glyph sizes keep the icon on the pixel grid: an odd icon inside an even
// square (or vice versa) lands on a half-pixel and reads as off-center.
const glyphSize = (squareSize: number): number => Math.max(8, Math.round((squareSize * 0.7) / 2) * 2)

export function PropertyTypeIcon({ size, type }: { size?: number | string; type: PropertyType }) {
  const match = typeIcons[type]

  if (!match) {
    console.warn(`No icon match for property type ${type}`)
    return null
  }

  const { className, icon: Icon } = match

  return (
    <PropertyIconSquare className={className} size={size}>
      <Icon size={glyphSize(toNumericSize(size))} strokeWidth={2.5} />
    </PropertyIconSquare>
  )
}
