import { AsyncLocalStorage } from 'async_hooks'

export interface WorkspaceContext {
  planId?: string
  isSubscriptionCancelled?: boolean
}

// @ts-expect-error outdated ts declrations
export const WorkspaceContextStorage = new AsyncLocalStorage<WorkspaceContext>({})
