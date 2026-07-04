// eslint-disable-next-line @typescript-eslint/no-require-imports
import Joi = require('joi')

import {
  booleanValueSchema,
  datetimeValueSchema,
  nullValueSchema,
  numberValueSchema,
  propertySchema,
  stringValueSchema
} from '@/core/property/validation/schemas/property.schema'

// Each alternative requires its payload key (`properties` / `data`): the pipe validates
// with allowUnknown, so without this a body with a mis-named payload field (e.g.
// `{ label, payload: {...} }`) passed validation empty and died in Cypher as a raw 500.
export const createEntitySchema = Joi.alternatives().try(
  Joi.object({
    label: Joi.string(),
    properties: Joi.array().items(propertySchema).required(),
    options: Joi.object({
      suggestTypes: Joi.boolean().optional(),
      skipEmptyValues: Joi.boolean().optional(),
      mergeStrategy: Joi.string().valid('append', 'rewrite').optional(),
      mergeBy: Joi.array().items(Joi.string().min(1).max(100)).optional()
    }).optional()
  }),
  Joi.object({
    label: Joi.string(),
    data: Joi.object()
      .pattern(
        Joi.string().min(1).max(100),
        Joi.alternatives().try(
          nullValueSchema,
          booleanValueSchema,
          datetimeValueSchema,
          numberValueSchema,
          stringValueSchema
        )
      )
      .required(),
    options: Joi.object({
      suggestTypes: Joi.boolean().optional(),
      skipEmptyValues: Joi.boolean().optional(),
      mergeStrategy: Joi.string().valid('append', 'rewrite').optional(),
      mergeBy: Joi.array().items(Joi.string().min(1).max(100)).optional()
    }).optional()
  })
)
