import type { ComponentPropsWithoutRef } from 'react'

import { useStore } from '@nanostores/react'

import { Button } from '~/elements/Button'
import { useCheckoutMutation } from '~/features/billing/hooks/useBillingHooks'
import { $router, isProjectPage } from '~/lib/router.ts'
import { $currentProjectId } from '~/features/projects/stores/id.ts'
import { trackUpgradeClicked } from '~/lib/analytics'

export const CheckoutButton = ({
  children = 'Checkout',
  priceId,
  planName,
  billingPeriod,
  loading: loadingProp,
  ...props
}: ComponentPropsWithoutRef<typeof Button> & {
  priceId: string
  planName?: string
  billingPeriod?: 'monthly' | 'annual'
}) => {
  const { mutate: checkout, isPending: checkoutInProgress } = useCheckoutMutation()

  const loading = loadingProp || checkoutInProgress

  const page = useStore($router)
  const currentProjectId = useStore($currentProjectId)

  const projectId = isProjectPage(page) ? page?.params.id : (currentProjectId ?? undefined)

  return (
    <Button
      {...props}
      loading={loading}
      onClick={() => {
        trackUpgradeClicked({ targetPlan: planName, billingPeriod })
        checkout({ priceId, returnUrl: window.location.href, projectId })
      }}
      role="link"
    >
      {children}
    </Button>
  )
}
