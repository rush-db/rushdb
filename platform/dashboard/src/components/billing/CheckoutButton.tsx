import type { ComponentPropsWithoutRef } from 'react'

import { useStore } from '@nanostores/react'

import { Button } from '~/elements/Button'
import { $checkout } from '~/features/billing/stores/checkout'
import { $router, isProjectPage } from '~/lib/router.ts'
import { $currentProjectId } from '~/features/projects/stores/id.ts'

export const CheckoutButton = ({
  children = 'Checkout',
  priceId,
  loading: loadingProp,
  ...props
}: ComponentPropsWithoutRef<typeof Button> & {
  priceId: string
}) => {
  const { mutate: checkout, loading: checkoutInProgress } = useStore($checkout)

  const loading = loadingProp || checkoutInProgress

  const page = useStore($router)
  const currentProjectId = useStore($currentProjectId)

  const projectId = isProjectPage(page) ? page?.params.id : (currentProjectId ?? undefined)

  return (
    <Button
      {...props}
      loading={loading}
      onClick={() => checkout({ priceId, returnUrl: window.location.href, projectId })}
      role="link"
    >
      {children}
    </Button>
  )
}
