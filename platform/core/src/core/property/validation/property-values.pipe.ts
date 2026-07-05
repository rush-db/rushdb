import { ArgumentMetadata, BadRequestException } from '@nestjs/common'
import { Injectable, PipeTransform } from '@nestjs/common'
import { z, ZodError } from 'zod'

import { checkTypeAndNameUniqueness } from '@/common/utils/checkTypeAndNameUniqueness'
import { formatErrorMessage } from '@/common/validation/utils'
import { normalizeRecord } from '@/core/common/normalizeRecord'
import { CreateEntityDto, CreateEntityDtoSimple } from '@/core/entity/dto/create-entity.dto'
import { EditEntityDto, EditEntityDtoSimple } from '@/core/entity/dto/edit-entity.dto'
import { TPropertySingleValue } from '@/core/property/property.types'
import { normalizeProperties, splitValueBySeparator } from '@/core/property/property.utils'
import {
  datetimeArraySchema,
  nullArraySchema,
  stringArraySchema
} from '@/core/property/validation/schemas/property.schema'

const schemasMap = {
  // Separator-split values are raw strings (parseValue converts them after validation),
  // so numeric strings must be accepted here — unlike the non-split number value schema.
  number: z.array(z.coerce.number()),
  string: stringArraySchema,
  datetime: datetimeArraySchema,
  null: nullArraySchema
}

@Injectable()
export class PropertyValuesPipe implements PipeTransform {
  handleError(error: Error | ZodError, metadata: any) {
    if (error instanceof ZodError) {
      throw new BadRequestException(formatErrorMessage(error, metadata))
    } else {
      throw new BadRequestException(error)
    }
  }
  transform(
    value: CreateEntityDto | CreateEntityDtoSimple | EditEntityDto | EditEntityDtoSimple,
    metadata: ArgumentMetadata
  ) {
    if (metadata.type === 'body' && 'properties' in value) {
      if (!checkTypeAndNameUniqueness(value.properties)) {
        throw new BadRequestException(`Duplicate name found with different types.`)
      }

      value.properties.forEach((property) => {
        if (property.valueSeparator) {
          const values = splitValueBySeparator(
            property.value as TPropertySingleValue,
            property.valueSeparator
          )
          const result = schemasMap[property.type].safeParse(values)
          if (!result.success) {
            this.handleError(result.error, metadata)
          }
        }
      })

      return { ...value, properties: normalizeProperties(value.properties) }
    } else if (metadata.type === 'body' && 'data' in value) {
      // @TODO: Implement schema schema validation https://github.com/rush-db/rushdb/issues/43
      const normalized = normalizeRecord(value as CreateEntityDtoSimple)
      const passthrough: Pick<CreateEntityDtoSimple, 'options' | 'vectors'> = {}

      // Preserve options (e.g., mergeBy/mergeStrategy for upsert) while normalizing `data` to `properties`.
      if ((value as CreateEntityDtoSimple).options) {
        passthrough.options = (value as CreateEntityDtoSimple).options
      }

      // Preserve inline vectors so the service can write them after the record is persisted.
      if ((value as CreateEntityDtoSimple).vectors?.length) {
        passthrough.vectors = (value as CreateEntityDtoSimple).vectors
      }

      return { ...normalized, ...passthrough }
    }
    return value
  }
}
