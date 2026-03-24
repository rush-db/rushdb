import type { ChangeEvent } from 'react'

import { action, atom } from 'nanostores'

export const $projectsQuery = atom<string>('')

export const filterProjects = action(
  $projectsQuery,
  'filterProjects',
  (store, event: ChangeEvent<HTMLInputElement>) => {
    store.set(event.target.value)
  }
)
