import { loadStripe } from '@stripe/stripe-js'

import type { ApiParams } from '~/lib/api'

import { $currentWorkspace } from '~/features/workspaces/stores/current-workspace'
import { api } from '~/lib/api'
import { createMutator } from '~/lib/fetcher'
import { $platformSettings } from '~/features/auth/stores/settings.ts'

export const $checkout = createMutator({
  async fetcher(body: ApiParams<typeof api.billing.createSession>) {
    if (!$platformSettings.get()?.data?.selfHosted) {
      const stripePromise = loadStripe(import.meta.env.VITE_STRIPE_PUBLIC_KEY)

      const stripe = await stripePromise

      if (!stripe) {
        // TODO: change error message
        throw new Error('Stripe failed to initialize')
      }

      const session = await api.billing.createSession(body)

      const { error } = await stripe.redirectToCheckout({
        sessionId: session.id
      })

      if (error) {
        throw error
      }
    }
  },
  invalidates: [$currentWorkspace]
})
