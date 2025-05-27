import React from 'react'
import type { TooltipRenderProps } from 'react-joyride'
import { Button } from '~/elements/Button'
import { IconButton } from '~/elements/IconButton'
import { X } from 'lucide-react'

export function CustomTooltip({
  step,
  backProps,
  closeProps,
  primaryProps,
  skipProps,
  continuous,
  index,
  tooltipProps
}: TooltipRenderProps) {
  const { noBack, noNext } = (step.data as any) || {}

  return (
    <div {...tooltipProps} className="bg-fill2 text-content relative max-w-sm rounded-lg p-6 shadow-lg">
      <IconButton
        {...closeProps}
        aria-label="Close tour"
        variant="ghost"
        className="text-content2 absolute right-2 top-2"
        size="small"
      >
        <X />
      </IconButton>

      <div>{step.content}</div>

      <div className="mt-4 flex items-center justify-between">
        {skipProps && (
          <Button size="small" variant="ghost" {...skipProps}>
            {skipProps.title}
          </Button>
        )}
        <div className="flex gap-2">
          {!noBack && index > 0 && backProps && (
            <Button size="small" variant="ghost" {...backProps}>
              {backProps.title}
            </Button>
          )}
          {!noNext && continuous && primaryProps && (
            <Button size="small" variant="accent" {...primaryProps}>
              {primaryProps.title}
            </Button>
          )}
          {noNext && (
            <Button size="small" variant="accent" {...closeProps}>
              Got it
            </Button>
          )}
        </div>
      </div>
    </div>
  )
}
