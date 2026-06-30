import React from 'react'
import type { TooltipRenderProps } from 'react-joyride'
import { Button } from '~/elements/Button'
import { IconButton } from '~/elements/IconButton'
import { X } from 'lucide-react'

type TooltipStepData = {
  clickTarget?: string
  noBack?: boolean
  noNext?: boolean
  waitForManualAction?: boolean
}

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
  const { noBack, noNext, waitForManualAction } = (step.data as TooltipStepData | undefined) || {}
  const isLastStep = primaryProps?.title === 'Last'
  const primaryTitle = primaryProps?.title === 'Last' ? 'Finish' : primaryProps?.title

  return (
    <div {...tooltipProps} className="bg-fill3 text-content relative max-w-sm rounded-lg p-6 shadow-lg">
      {!waitForManualAction && (
        // The X dismisses onboarding entirely — same as Skip. It spreads
        // skipProps (not closeProps) so react-joyride emits STATUS.SKIPPED,
        // which OnboardingTour persists as onboardingStatus: 'skipped' and
        // stops the tour. closeProps alone is swallowed by the controlled
        // stepIndex/run and does nothing. Falls back to closeProps if the
        // skip button is ever disabled.
        <IconButton
          {...(skipProps ?? closeProps)}
          aria-label="Skip tour"
          variant="ghost"
          className="text-content2 absolute right-2 top-2"
          size="small"
        >
          <X />
        </IconButton>
      )}

      <div>{step.content}</div>

      <div className="mt-4 flex items-center justify-between">
        {skipProps && !isLastStep && (
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
            <Button size="small" variant="primary" {...primaryProps}>
              {primaryTitle}
            </Button>
          )}
          {noNext && !waitForManualAction && (
            <Button
              size="small"
              variant="primary"
              {...closeProps}
              onClick={(e: React.MouseEvent) => {
                const data = step.data as TooltipStepData | undefined
                const selector = data?.clickTarget ?? (typeof step.target === 'string' ? step.target : null)
                if (selector) {
                  const el = document.querySelector(selector)
                  if (el instanceof HTMLElement) el.click()
                }
                closeProps.onClick(e as React.MouseEvent<HTMLElement, MouseEvent>)
              }}
            >
              Got it
            </Button>
          )}
          {noNext && waitForManualAction && (
            <Button size="small" variant="primary" {...closeProps}>
              Got it
            </Button>
          )}
        </div>
      </div>
    </div>
  )
}
