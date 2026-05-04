import { IProjectProperties } from '@/dashboard/project/model/project.interface'

import type { ProjectRow } from '@/database/sql/schema/types'

export class ProjectEntity {
  constructor(private readonly projectNode: ProjectRow) {}

  getProperties(): IProjectProperties {
    return {
      id: this.projectNode.id,
      name: this.projectNode.name,
      created: this.projectNode.created,
      edited: this.projectNode.edited,
      description: this.projectNode.description,
      customDb: this.projectNode.customDb,
      status: this.projectNode.status as any,
      stats: this.projectNode.stats,
      ontologyCache: this.projectNode.ontologyCache,
      ontologyCachedAt: this.projectNode.ontologyCachedAt
    }
  }

  toJson() {
    return this.getProperties()
  }

  toJSON() {
    return this.getProperties()
  }
}
