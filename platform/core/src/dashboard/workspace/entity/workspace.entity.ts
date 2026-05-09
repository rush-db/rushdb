import { IWorkspaceProperties } from '@/dashboard/workspace/model/workspace.interface'

import type { WorkspaceRow } from '@/database/sql/schema/types'

export class Workspace {
  // expose dataValues for legacy call sites that read workspace.dataValues.id etc.
  readonly dataValues: WorkspaceRow

  constructor(private readonly row: WorkspaceRow) {
    this.dataValues = row
  }

  getProperties(): IWorkspaceProperties {
    return {
      id: this.row.id,
      name: this.row.name,
      created: this.row.created,
      edited: this.row.edited,
      pendingInvites: undefined, // pendingInvites moved to workspace_invites table
      stats: this.row.stats
    }
  }

  toJson() {
    return this.getProperties()
  }
}
