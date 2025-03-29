import { Neogma } from 'neogma'
import { AsyncLocalStorage } from 'async_hooks'

export interface DbContext {
  projectId: string
  connection: Neogma
}

export const dbContextStorage = new AsyncLocalStorage<DbContext>()
