import { IProjectProperties } from '@/dashboard/project/model/project.interface'

export class ProjectEntity {
  constructor(
    private readonly id: string,
    private readonly name: string,
    private readonly created: string,
    private readonly description?: string,
    private readonly edited?: string,
    private readonly customDb?: string
  ) {}

  getProperties(): IProjectProperties {
    return {
      id: this.id,
      name: this.name,
      created: this.created,
      edited: this.edited,
      description: this.description,
      customDb: this.customDb
    }
  }

  toJson() {
    return this.getProperties()
  }
}
