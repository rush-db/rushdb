import { KuOperation } from '@/core/ku-events/ku-events.constants'

export interface KuOperationEvent {
  /** Workspace ID — the billing tenant (one subscription per workspace) */
  workspaceId: string
  /** Project ID — attribution/analytics (multiple projects per workspace) */
  projectId: string
  operation: KuOperation
  metadata?: Record<string, unknown>
  timestamp: number
}
