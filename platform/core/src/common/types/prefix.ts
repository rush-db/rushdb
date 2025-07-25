import { PlanPrefix } from '@/common/constants'

export type PlanType = keyof typeof PlanPrefix

export interface ServerSettings {
  selfHosted: boolean
  customDB?: boolean
  managedDB?: boolean
  canceled?: boolean
}

export type MixedPlanProperties = ServerSettings & { plan: PlanType }

export type MixedTypeResult = [MixedPlanProperties | null, string]
