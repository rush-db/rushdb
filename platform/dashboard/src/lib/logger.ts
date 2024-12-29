import { logger } from '@nanostores/logger'

import { $user } from '~/features/auth/stores/user'
import { $currentProjectFilters } from '~/features/projects/stores/current-project'

import { $searchParams } from './router'

logger({
  $searchParams,
  $user,
  $currentProjectFilters
})
