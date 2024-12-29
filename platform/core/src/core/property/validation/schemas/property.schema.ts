import Joi = require('joi')
import { AnySchema } from 'joi'

import { ISO_8601_REGEX, NUMERIC_REGEX } from '@/core/common/constants'
import {
  PROPERTY_TYPE_BOOLEAN,
  PROPERTY_TYPE_DATETIME,
  PROPERTY_TYPE_NULL,
  PROPERTY_TYPE_NUMBER,
  PROPERTY_TYPE_STRING
} from '@/core/property/property.constants'

const datetimeSchema = Joi.string().regex(ISO_8601_REGEX).messages({
  'string.pattern.base': '{{#label}} with value {:[.]} fails to match ISO8601 DateTime'
})

export const numericStringSchema = Joi.string()
  .regex(NUMERIC_REGEX)
  .messages({
    'string.pattern.base': '{{#label}} with value {:[.]} is not a numeric value'
  })
  .required()

export const fieldValueSchema = Joi.alternatives(numericStringSchema, datetimeSchema.required())
const JoiArraySchema = (schema: AnySchema) => Joi.array().items(schema).required()

export const stringArraySchema = JoiArraySchema(Joi.string().required())
export const datetimeArraySchema = JoiArraySchema(datetimeSchema.required())
export const numberArraySchema = JoiArraySchema(Joi.number().required())
export const nullArraySchema = JoiArraySchema(Joi.string().valid(null).required())
export const booleanArraySchema = JoiArraySchema(Joi.boolean().required())
export const emptyArraySchema = Joi.array().length(0)

export const nullValueSchema = Joi.alternatives().try(
  Joi.string().valid(null).required(),
  nullArraySchema,
  emptyArraySchema
)
export const booleanValueSchema = Joi.alternatives().try(
  Joi.boolean().required(),
  booleanArraySchema,
  emptyArraySchema
)
export const datetimeValueSchema = Joi.alternatives().try(
  datetimeSchema.required(),
  datetimeArraySchema,
  emptyArraySchema
)
export const numberValueSchema = Joi.alternatives().try(
  Joi.number().required(),
  numberArraySchema,
  emptyArraySchema
)
export const stringValueSchema = Joi.alternatives()
  .try(Joi.string().required(), stringArraySchema, emptyArraySchema)
  .allow('')

export const propertySchema = Joi.object({
  type: Joi.string()
    .valid(
      PROPERTY_TYPE_STRING,
      PROPERTY_TYPE_DATETIME,
      PROPERTY_TYPE_BOOLEAN,
      PROPERTY_TYPE_NUMBER,
      PROPERTY_TYPE_NULL
    )
    .required(),
  name: Joi.string().required(),
  value: Joi.alternatives()
    .conditional('type', {
      switch: [
        {
          is: PROPERTY_TYPE_BOOLEAN,
          then: booleanValueSchema
        },
        {
          is: PROPERTY_TYPE_NULL,
          then: nullValueSchema
        },
        {
          is: PROPERTY_TYPE_DATETIME,
          then: Joi.alternatives().conditional('valueSeparator', {
            is: Joi.string().exist(),
            then: datetimeSchema.required(),
            otherwise: datetimeValueSchema
          })
        },
        {
          is: PROPERTY_TYPE_STRING,
          then: Joi.alternatives().conditional('valueSeparator', {
            is: Joi.string().exist(),
            then: Joi.string().required(),
            otherwise: stringValueSchema
          })
        },
        {
          is: PROPERTY_TYPE_NUMBER,
          then: Joi.alternatives().conditional('valueSeparator', {
            is: Joi.string().exist(),
            then: Joi.string().required(),
            otherwise: numberValueSchema
          })
        }
      ]
    })
    .required(),
  valueSeparator: Joi.string().optional(),
  metadata: Joi.object().optional()
})
