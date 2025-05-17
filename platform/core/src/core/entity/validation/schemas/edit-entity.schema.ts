import Joi = require('joi')

import {
  booleanValueSchema,
  datetimeValueSchema,
  nullValueSchema,
  numberValueSchema,
  propertySchema,
  stringValueSchema
} from '@/core/property/validation/schemas/property.schema'

export const editEntitySchema = Joi.alternatives().try(
  Joi.object({
    label: Joi.string(),
    properties: Joi.array().items(propertySchema).optional()
  }),
  Joi.object({
    label: Joi.string(),
    data: Joi.object().pattern(
      Joi.string().min(1).max(100),
      Joi.alternatives().try(
        nullValueSchema,
        booleanValueSchema,
        datetimeValueSchema,
        numberValueSchema,
        stringValueSchema
      )
    ),
    options: Joi.object({ suggestTypes: Joi.boolean().optional() }).optional()
  })
)
