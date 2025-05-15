import { Neogma } from 'neogma'

import { AsyncLocalStorage } from 'async_hooks'

export interface DbContext {
  projectId: string
  connection: Neogma
}
// @ts-expect-error outdated ts declrations
export const dbContextStorage = new AsyncLocalStorage<DbContext>({})
