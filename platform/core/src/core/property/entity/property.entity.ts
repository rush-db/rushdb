import { TPropertyValue } from '@/core/property/property.types'

import { TPropertyInstance, TPropertyPropertiesNormalized } from '../model/property.interface'

export class Property {
  constructor(private readonly instance: TPropertyInstance, readonly value?: TPropertyValue) {}

  getProperties(): TPropertyPropertiesNormalized {
    return {
      id: this.instance.dataValues.id,
      name: this.instance.dataValues.name,
      type: this.instance.dataValues.type,
      projectId: this.instance.dataValues.projectId,
      metadata: this.instance.dataValues.metadata,
      value: this.value
    }
  }

  toJson() {
    return this.getProperties()
  }
}
