import type { ISO8601 } from '~/types'

export type ProjectToken = {
  created: ISO8601
  description?: string
  expiration: number | string
  expired?: boolean // exists on list
  id: string
  name: string
  noExpire?: boolean
  value?: string // exists when created
  level?: 'read' | 'write'
  issuedBy?: string // 'manual' | 'oauth_exchange'
}

// A token as returned by the workspace-wide listing — carries the project it grants
// access to, since a workspace key list spans every project in the workspace.
export type WorkspaceToken = ProjectToken & {
  project: { id: string; name: string }
}
