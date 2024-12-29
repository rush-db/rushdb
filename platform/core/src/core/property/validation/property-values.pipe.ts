import { ArgumentMetadata, BadRequestException } from '@nestjs/common'
import { Injectable, PipeTransform } from '@nestjs/common'
import Joi = require('joi')

import { checkTypeAndNameUniqueness } from '@/common/utils/checkTypeAndNameUniqueness'
import { formatErrorMessage } from '@/common/validation/utils'
import { CreateEntityDto, CreateEntityDtoSimple } from '@/core/entity/dto/create-entity.dto'
import { EditEntityDto } from '@/core/entity/dto/edit-entity.dto'
import { TPropertySingleValue } from '@/core/property/property.types'
import { normalizeProperties, splitValueBySeparator } from '@/core/property/property.utils'
import {
  datetimeArraySchema,
  nullArraySchema,
  numberArraySchema,
  stringArraySchema
} from '@/core/property/validation/schemas/property.schema'

const schemasMap = {
  number: numberArraySchema,
  string: stringArraySchema,
  datetime: datetimeArraySchema,
  null: nullArraySchema
}

@Injectable()
export class PropertyValuesPipe implements PipeTransform {
  handleError(error: Error | Joi.ValidationError, metadata: any) {
    if (Joi.isError(error)) {
      throw new BadRequestException(formatErrorMessage(error, metadata))
    } else {
      throw new BadRequestException(error)
    }
  }
  transform(value: CreateEntityDto | CreateEntityDtoSimple | EditEntityDto, metadata: ArgumentMetadata) {
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
          const { error } = schemasMap[property.type].validate(values)
          if (error) {
            this.handleError(error, metadata)
          }
        }
      })

      return { ...value, properties: normalizeProperties(value.properties) }
    }
    return value
  }
}
