import React, { useEffect, useRef } from 'react'
import Joyride, { CallBackProps, EVENTS, STATUS } from 'react-joyride'
import { useStore } from '@nanostores/react'
import { $user, updateUser } from '~/features/auth/stores/user'
import { $router, openRoute, projectRoutes, routes } from '~/lib/router'
import { steps, keys } from '~/features/tour/config/steps'
import {
  $tourStep,
  $tourRunning,
  $tourEffective,
  $tourAllowed,
  setTourStep
} from '~/features/tour/stores/tour'
import { CustomTooltip } from '~/features/tour/components/CustomTooltip'
import { $currentProjectId } from '~/features/projects/stores/id'
import { $platformSettings } from '~/features/auth/stores/settings.ts'

export function OnboardingTour() {
  const page = useStore($router) as { route: keyof typeof routes }
  const user = useStore($user)
  const projectId = useStore($currentProjectId)
  const currentKey = useStore($tourStep)
  const run = useStore($tourEffective)
  const platformSettings = useStore($platformSettings)
  const { mutate: updateSettings } = useStore(updateUser)
  const isAllowed = useStore($tourAllowed)
  const initializedRef = useRef(false)
  const isOwner = user.currentScope?.role === 'owner'

  useEffect(() => {
    if (user && !isOwner && !user.isLoggedIn && !platformSettings.data?.selfHosted) {
      return
    }

    if (initializedRef.current) return

    const settings = user.settings || ''

    let status: 'skipped' | 'finished' | 'active' | undefined

    try {
      status = JSON.parse(settings)?.onboardingStatus as 'skipped' | 'finished' | 'active'
    } catch {
      status = undefined
    }

    if (!status) {
      updateSettings({
        settings: JSON.stringify({
          onboardingStatus: 'active'
        })
      })

      initializedRef.current = true

      $tourRunning.set(true)
    } else if (status === 'active') {
      $tourRunning.set(true)
    } else {
      $tourRunning.set(false)
    }
  }, [user])

  useEffect(() => {
    const def = steps.find((s) => (s.data as any).key === currentKey)
    if (!def) return

    if (page.route === (def.data as any).route && isAllowed) {
      $tourRunning.set(true)
    } else {
      $tourRunning.set(false)
    }
  }, [page.route, currentKey])

  const handleCallback = ({ status, type, action, index, step }: CallBackProps) => {
    if (status === STATUS.SKIPPED) {
      updateSettings({
        settings: JSON.stringify({
          onboardingStatus: 'skipped'
        })
      })
      $tourRunning.set(false)
      return
    }

    if (action === 'next' && index === steps.length - 1) {
      updateSettings({
        settings: JSON.stringify({
          onboardingStatus: 'finished'
        })
      })
      $tourRunning.set(false)
      return
    }

    if (type === EVENTS.STEP_AFTER) {
      const data = (steps[index].data as any) || {}
      const nextStep = (steps[index + 1].data as any) || {}

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
        setTourStep(nextKey, false)
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
        setTourStep(prevKey, true)
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
