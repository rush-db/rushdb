import { IWorkspaceProperties, TWorkspaceInstance } from '@/dashboard/workspace/model/workspace.interface'

export class Workspace {
  constructor(private readonly node: TWorkspaceInstance) {}

  getProperties(): IWorkspaceProperties {
    return {
      id: this.node.dataValues.id,
      name: this.node.dataValues.name,
      created: this.node.dataValues.created,
      edited: this.node.dataValues.edited,
      pendingInvites: this.node.dataValues.pendingInvites,
      stats: this.node.dataValues.stats
    }
  }

  toJson() {
    return this.getProperties()
  }
}
