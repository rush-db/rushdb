import React, { useEffect } from 'react'
import Joyride, { CallBackProps, EVENTS, STATUS } from 'react-joyride'
import { useStore } from '@nanostores/react'
import { updateUser } from '~/features/auth/stores/user'
import { $router, openRoute, projectRoutes, routes } from '~/lib/router'
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

export function OnboardingTour() {
  const page = useStore($router) as { route: keyof typeof routes }
  const projectId = useStore($currentProjectId)
  const currentKey = useStore($tourStep)
  const run = useStore($tourEffective)
  const { mutate: updateSettings } = useStore(updateUser)
  const isAllowed = useStore($tourAllowed)

  const currentStep = steps.find((s) => (s.data as any).key === currentKey)
  const targetSelector = (currentStep?.target as string) || ''
  const stepReady = useWaitForSelectorStable(targetSelector)

  useEffect(() => {
    const def = steps.find((s) => (s.data as any).key === currentKey)
    if (!def) {
      $tourRunning.set(false)
      return
    }

    const { route: stepRoute } = def.data as any
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
      $tourRunning.set(false)
      return
    }

    if (type === EVENTS.STEP_AFTER) {
      const data = (steps[index].data as any) || {}

      if (action === 'next' && !data.nextShouldBeManuallySet) {
        const nextKey = keys[index + 1]
        if (data.redirectTo) {
          const route = data.redirectTo as keyof typeof projectRoutes
          if (projectRoutes[route] && projectId) {
            openRoute(route, { id: projectId })
          } else {
            openRoute(route as keyof typeof routes)
          }
        }
        setTourStep(nextKey)
      }

      if (action === 'prev' && !data.noBack) {
        const prevKey = keys[index - 1]
        const prevData = (steps[index - 1]?.data as any) || {}
        const backRoute = prevData.route as keyof typeof routes
        if (projectRoutes[backRoute as keyof typeof projectRoutes] && projectId) {
          openRoute(backRoute as keyof typeof projectRoutes, { id: projectId })
        } else {
          openRoute(backRoute)
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
