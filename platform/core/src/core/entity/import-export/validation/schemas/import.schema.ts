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
  }).optional(),
  parseConfig: Joi.object({
    delimiter: Joi.string().max(5).optional(),
    header: Joi.boolean().optional(),
    skipEmptyLines: Joi.alternatives().try(Joi.boolean(), Joi.string().valid('greedy')).optional(),
    dynamicTyping: Joi.boolean().optional(),
    quoteChar: Joi.string().max(3).optional(),
    escapeChar: Joi.string().max(3).optional(),
    newline: Joi.string().max(4).optional()
  }).optional()
})
