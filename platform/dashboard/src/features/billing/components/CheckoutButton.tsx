import type { ComponentPropsWithoutRef } from 'react'

import { useStore } from '@nanostores/react'

import type { PlanId } from '~/features/billing/types'

import { Button } from '~/elements/Button'
import { $checkout } from '~/features/billing/stores/checkout'
import { $currentPeriod } from '~/features/billing/stores/plans'

export const CheckoutButton = ({
  children = 'Checkout',
  id,
  loading: loadingProp,
  ...props
}: ComponentPropsWithoutRef<typeof Button> & {
  id: PlanId
}) => {
  const { mutate: checkout, loading: checkoutInProgress } = useStore($checkout)

  const period = useStore($currentPeriod)

  const loading = loadingProp || checkoutInProgress

  return (
    <Button
      {...props}
      loading={loading}
      onClick={() => checkout({ id, period, returnUrl: window.location.href })}
      role="link"
    >
      {children}
    </Button>
  )
}
