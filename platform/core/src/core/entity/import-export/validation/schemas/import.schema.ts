import Joi = require('joi')

export const importJsonSchema = Joi.object({
  label: Joi.string(),
  data: Joi.alternatives().try(Joi.object(), Joi.array().items(Joi.object())),
  options: Joi.object({
    suggestTypes: Joi.boolean().optional()
  }).optional()
})

export const importCsvSchema = Joi.object({
  label: Joi.string(),
  data: Joi.string(),
  options: Joi.object({
    suggestTypes: Joi.boolean().optional()
  }).optional()
})
