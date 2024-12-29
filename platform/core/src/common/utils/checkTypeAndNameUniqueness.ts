import { PropertyDto } from '@/core/property/dto/property.dto'

export const checkTypeAndNameUniqueness = (properties: PropertyDto[]) => {
  const nameMap = new Map<string, string>()

  for (const property of properties) {
    if (nameMap.has(property.name)) {
      const existingType = nameMap.get(property.name)
      if (existingType !== property.type) {
        // throw new Error(
        //     `Duplicate name '${property.name}' found with different types: '${existingType}' and '${property.type}'.`
        // );
        return false
      }
    } else {
      nameMap.set(property.name, property.type)
    }
  }

  return true
}
