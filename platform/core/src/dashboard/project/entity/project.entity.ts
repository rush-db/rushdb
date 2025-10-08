import { IProjectProperties } from '@/dashboard/project/model/project.interface'

export class ProjectEntity {
  constructor(private readonly projectNode: IProjectProperties) {}

  getProperties(): IProjectProperties {
    return {
      id: this.projectNode.id,
      name: this.projectNode.name,
      created: this.projectNode.created,
      edited: this.projectNode.edited,
      description: this.projectNode.description,
      customDb: this.projectNode.customDb,
      managedDbRegion: this.projectNode.managedDbRegion,
      managedDbTier: this.projectNode.managedDbTier,
      status: this.projectNode.status,
      stats: this.projectNode.stats
    }
  }

  toJson() {
    return this.getProperties()
  }
}
