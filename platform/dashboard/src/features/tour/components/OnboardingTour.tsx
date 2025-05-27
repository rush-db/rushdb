import React, { useEffect } from 'react'
import Joyride, { CallBackProps, EVENTS, STATUS } from 'react-joyride'
import { useStore } from '@nanostores/react'
import { $user } from '~/features/auth/stores/user'
import { $router, openRoute, projectRoutes, routes } from '~/lib/router'
import { steps, keys } from '~/features/tour/config/steps'
import { $tourStep, $tourRunning } from '~/features/tour/stores/tour'
import { CustomTooltip } from '~/features/tour/components/CustomTooltip'
import { $currentProjectId } from '~/features/projects/stores/id'
import { $platformSettings } from '~/features/auth/stores/settings.ts'

export function OnboardingTour() {
  const page = useStore($router) as { route: keyof typeof routes }
  const user = useStore($user)
  const projectId = useStore($currentProjectId)
  const currentKey = useStore($tourStep)
  const run = useStore($tourRunning)
  const platformSettings = useStore($platformSettings)

  useEffect(() => {
    if (user.isLoggedIn && !platformSettings?.data?.selfHosted) $tourRunning.set(true)
  }, [user.isLoggedIn])

  useEffect(() => {
    const def = steps.find((s) => (s.data as any).key === currentKey)
    if (!def) return
    $tourRunning.set(page.route === (def.data as any).route)
  }, [page.route, currentKey])

  const handleCallback = ({ status, type, action, index }: CallBackProps) => {
    if (status === STATUS.SKIPPED || status === STATUS.FINISHED) {
      $tourRunning.set(false)
      // TODO: mutate setting user.onboardingSkipped = true
      return
    }

    if (type === EVENTS.STEP_AFTER) {
      const data = (steps[index].data as any) || {}

      // Next
      if (action === 'next' && !data.nextShouldBeManuallySet) {
        const nextKey = keys[index + 1]
        if (data.redirectTo) {
          const route = data.redirectTo as keyof typeof projectRoutes

          if (projectRoutes[route] && projectId) {
            openRoute(route as keyof typeof projectRoutes, { id: projectId })
          } else {
            openRoute(route as keyof typeof routes)
          }
        }
        $tourStep.set(nextKey)
      }

      // Back
      if (action === 'prev' && !data.noBack) {
        const prevKey = keys[index - 1]
        const prevData = (steps[index - 1]?.data as any) || {}
        const backRoute = prevData.route as keyof typeof routes

        if (projectRoutes[backRoute as keyof typeof projectRoutes] && projectId) {
          openRoute(backRoute as keyof typeof projectRoutes, { id: projectId })
        } else {
          openRoute(backRoute)
        }
        $tourStep.set(prevKey)
      }
    }
  }

  return (
    <Joyride
      steps={steps}
      run={run}
      stepIndex={keys.indexOf(currentKey)}
      continuous
      showSkipButton
      disableOverlay
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
