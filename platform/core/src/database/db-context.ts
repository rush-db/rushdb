import { Neogma } from 'neogma'

import { AsyncLocalStorage } from 'async_hooks'

export interface DbContext {
  projectId: string
  connection: Neogma
  externalConnection?: Neogma
}
// @ts-expect-error outdated ts declarations
export const dbContextStorage = new AsyncLocalStorage<DbContext>({})
