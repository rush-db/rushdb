import { MixedPlanProperties, MixedTypeResult, PlanType, ServerSettings } from '@/common/types/prefix'
import { PlanPrefix, RESERVED_FLAGS_COUNT } from '@/common/constants'

export function attachMixedProperties(plan: PlanType, settings?: Partial<ServerSettings>): string {
  const { customDB = false, selfHosted = false } = settings ?? {}

  // Compute features bits customDB → (managedDB deprecated, always 0) → selfHosted
  const bitsCore = [customDB ? '1' : '0', '0', selfHosted ? '1' : '0'].join('')

  // Attach reserved bits
  const bitsReserved = '0'.repeat(RESERVED_FLAGS_COUNT)

  // Build: <planPrefix>_<bitsCore+bitsReserved>_
  return `${PlanPrefix[plan]}_${bitsCore + bitsReserved}_`
}

export function extractMixedPropertiesFromToken(prefixedToken: string): MixedTypeResult {
  // Try such token format XX_YYYYYY_ZZZ
  const reg = /^([a-z]{2})_([01]{3}\d{0,})_(.+)$/

  const matchedToken = prefixedToken.match(reg)

  if (!matchedToken) {
    // Working with old token
    return [null, prefixedToken]
  }

  const [, prefix, bits, rawToken] = matchedToken

  // Get user prefixed plan
  const planEntry = (Object.entries(PlanPrefix) as [PlanType, string][]).find(
    ([_, currentPrefix]) => currentPrefix === prefix
  )

  const plan = planEntry[0]

  // Build feature flags (managedDB deprecated, second bit ignored for backward compatibility)
  const [bCustomDb, , bSelfHosted] = bits.split('')

  const settings: ServerSettings = {
    customDB: bCustomDb === '1',
    selfHosted: bSelfHosted === '1'
  }

  return [{ plan, ...settings }, rawToken]
}

export function getNormalizedPrefix(prefixString: string): MixedPlanProperties | null {
  const reg = /^([a-z]{2})_([01]{3}\d{0,})_$/

  const matchedPrefix = prefixString.match(reg)

  if (!matchedPrefix) {
    // Working with old token
    return null
  }

  const [, prefix, bits] = matchedPrefix

  // Get user prefixed plan
  const planEntry = (Object.entries(PlanPrefix) as [PlanType, string][]).find(
    ([_, currentPrefix]) => currentPrefix === prefix
  )

  const plan = planEntry[0]

  // Build feature flags (managedDB deprecated, second bit ignored for backward compatibility)
  const [bCustomDb, , bSelfHosted] = bits.split('')

  const settings: ServerSettings = {
    customDB: bCustomDb === '1',
    selfHosted: bSelfHosted === '1'
  }

  return { plan, ...settings }
}

export function getPreparedToken(rawToken: string, prefixValue?: string) {
  return prefixValue ? `${prefixValue}${rawToken}` : rawToken
}

export function getPrefixedPlan(planId: string | undefined): PlanType {
  if (!planId) {
    return 'initial'
  } else if (planId === 'start') {
    return 'extended'
  } else {
    return 'fullFeatured'
  }
}
