import type { PlanId } from '~/features/billing/types'
import type { ISO8601 } from '~/types'

export type Workspace = {
  created: ISO8601
  id: string
  limits: {
    projects: number
  }
  name: string
  planId?: PlanId
  validTill?: ISO8601
  isSubscriptionCancelled?: boolean
}
