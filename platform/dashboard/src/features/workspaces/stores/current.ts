import { persistentAtom } from '@nanostores/persistent'

import type { Workspace } from '../types'

export const $currentWorkspaceId = persistentAtom<Workspace['id'] | undefined>(
  'workspace:current',
  undefined
)
