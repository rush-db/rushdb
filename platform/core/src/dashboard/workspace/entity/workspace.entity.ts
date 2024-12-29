import { IWorkspaceProperties, TWorkspaceInstance } from '@/dashboard/workspace/model/workspace.interface'

export class Workspace {
  constructor(private readonly node: TWorkspaceInstance) {}

  getProperties(): IWorkspaceProperties {
    return {
      id: this.node.dataValues.id,
      name: this.node.dataValues.name,
      created: this.node.dataValues.created,
      edited: this.node.dataValues.edited,
      limits: this.node.dataValues.limits,
      planId: this.node.dataValues.planId,
      validTill: this.node.dataValues.validTill,
      isSubscriptionCancelled: this.node.dataValues.isSubscriptionCancelled
    }
  }

  toJson() {
    return this.getProperties()
  }
}
