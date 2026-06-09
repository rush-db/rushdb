import React, { useEffect } from 'react'
import type { CallBackProps } from 'react-joyride'
import Joyride, { EVENTS, STATUS } from 'react-joyride'
import { useStore } from '@nanostores/react'
import { useUpdateUserMutation } from '~/features/auth/hooks/useAuthMutations'
import type { routes } from '~/lib/router'
import { $router, openRoute, projectRoutes } from '~/lib/router'
import { steps, keys } from '~/features/tour/config/steps'
import {
  $tourStep,
  $tourRunning,
  $tourEffective,
  $tourAllowed,
  setTourStep
} from '~/features/tour/stores/tour'
import { CustomTooltip } from './CustomTooltip'
import { $currentProjectId } from '~/features/projects/stores/id'
import { useWaitForSelectorStable } from '~/features/tour/hooks/useWaitForSelector'
import type { TourStepKey } from '~/features/tour/types'

type TourStepData = {
  key: TourStepKey
  route: keyof typeof routes
  redirectTo?: keyof typeof routes
  nextShouldBeManuallySet?: boolean
  noBack?: boolean
  waitForManualAction?: boolean
}

function getStepData(step?: (typeof steps)[number]) {
  return step?.data as TourStepData | undefined
}

function isProjectRouteName(route: keyof typeof routes): route is keyof typeof projectRoutes {
  return route in projectRoutes
}

function fireOnboardingConfetti() {
  if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return

  void import('@tsparticles/confetti')
    .then(({ confetti }) =>
      confetti({
        count: 120,
        spread: 80,
        startVelocity: 45,
        decay: 0.9,
        gravity: 0.9,
        scalar: 1,
        ticks: 220,
        zIndex: 10001,
        colors: ['#C7F943', '#6366F1', '#FFFFFF', '#7DD3FC']
      })
    )
    .catch(() => undefined)
}

export function OnboardingTour() {
  const page = useStore($router) as { route: keyof typeof routes }
  const projectId = useStore($currentProjectId)
  const currentKey = useStore($tourStep)
  const run = useStore($tourEffective)
  const { mutateAsync: updateSettings } = useUpdateUserMutation()
  const isAllowed = useStore($tourAllowed)

  const currentStep = steps.find((step) => getStepData(step)?.key === currentKey)
  const targetSelector = typeof currentStep?.target === 'string' ? currentStep.target : ''
  const stepReady = useWaitForSelectorStable(targetSelector)

  useEffect(() => {
    const def = steps.find((step) => getStepData(step)?.key === currentKey)
    if (!def) {
      setTourStep('welcome', true)
      $tourRunning.set(false)
      return
    }

    const stepRoute = getStepData(def)?.route
    if (page.route === stepRoute && isAllowed && stepReady) {
      $tourRunning.set(true)
    } else {
      $tourRunning.set(false)
    }
  }, [page.route, currentKey, isAllowed, stepReady])

  const handleCallback = ({ status, type, action, index }: CallBackProps) => {
    if (status === STATUS.SKIPPED) {
      updateSettings({ settings: JSON.stringify({ onboardingStatus: 'skipped' }) })
      $tourRunning.set(false)
      return
    }

    if (action === 'next' && index === steps.length - 1) {
      updateSettings({ settings: JSON.stringify({ onboardingStatus: 'finished' }) })
      fireOnboardingConfetti()
      $tourRunning.set(false)
      return
    }

    if (type === EVENTS.STEP_AFTER) {
      const data = getStepData(steps[index])

      if (action === 'next' && data && !data.nextShouldBeManuallySet) {
        const nextKey = keys[index + 1]
        if (data.redirectTo) {
          const route = data.redirectTo
          if (isProjectRouteName(route) && projectId) {
            openRoute(route, { id: projectId })
          } else {
            openRoute(route)
          }
        }
        setTourStep(nextKey)
      }

      if (action === 'prev' && !data?.noBack) {
        const prevKey = keys[index - 1]
        const prevData = getStepData(steps[index - 1])
        const backRoute = prevData?.route
        if (backRoute && isProjectRouteName(backRoute) && projectId) {
          openRoute(backRoute, { id: projectId })
        } else if (backRoute) {
          openRoute(backRoute)
        } else {
          openRoute('home')
        }
        setTourStep(prevKey, true)
      }
    }
  }

  return (
    <Joyride
      steps={steps}
      run={run && stepReady}
      stepIndex={keys.indexOf(currentKey)}
      continuous
      showSkipButton
      disableOverlay
      disableScrollParentFix
      tooltipComponent={CustomTooltip}
      callback={handleCallback}
      styles={{
        options: {
          arrowColor: 'rgb(29,29,29)',
          backgroundColor: 'rgb(29,29,29)',
          overlayColor: 'rgba(0,0,0,0.5)',
          primaryColor: '#6366F1',
          textColor: '#F9FAFB',
          zIndex: 10000
        }
      }}
    />
  )
}
