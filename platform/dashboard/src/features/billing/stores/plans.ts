import { persistentAtom } from '@nanostores/persistent'

import type { PlanPeriod } from '~/features/billing/types'

export const $currentPeriod = persistentAtom<PlanPeriod>('billing:period', 'annual')
