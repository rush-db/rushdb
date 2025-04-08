import Joi = require('joi')

import {
  booleanValueSchema,
  datetimeValueSchema,
  nullValueSchema,
  numberValueSchema,
  propertySchema,
  stringValueSchema
} from '@/core/property/validation/schemas/property.schema'

export const upsertEntitySchema = Joi.alternatives().try(
  Joi.object({
    label: Joi.string(),
    properties: Joi.array().items(propertySchema).optional(),
    matchBy: Joi.array().items(Joi.string()).optional()
  }),
  Joi.object({
    label: Joi.string(),
    payload: Joi.object().pattern(
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
