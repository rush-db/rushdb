import { QueryClient } from '@tanstack/react-query'

import { toast } from '~/elements/Toast'
import { BillingErrorCodes } from '~/features/billing/constants'
import { $limitReachModalOpen } from '~/components/billing/LimitReachedDialog'

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 0,
      gcTime: 5 * 60 * 1000,
      retry: false,
      refetchOnWindowFocus: false
    },
    mutations: {
      onError(error) {
        if (error instanceof Error) {
          const status = (error as any).response?.status
          if (status === BillingErrorCodes.PaymentRequired) {
            $limitReachModalOpen.set(true)
            return
          }
          toast({ variant: 'danger', title: error.message })
        }
      }
    }
  }
})
