// eslint-disable-next-line @typescript-eslint/no-require-imports
import Joi = require('joi')

export const importJsonSchema = Joi.object({
  label: Joi.string(),
  data: Joi.alternatives().try(Joi.object(), Joi.array().items(Joi.object()), Joi.string()),
  format: Joi.string().valid('json', 'jsonl', 'ndjson').optional(),
  options: Joi.object({
    suggestTypes: Joi.boolean().optional(),
    convertNumericValuesToNumbers: Joi.boolean().optional(),
    capitalizeLabels: Joi.boolean().optional(),
    relationshipType: Joi.string().optional(),
    returnResult: Joi.boolean().optional(),
    mergeStrategy: Joi.string().valid('append', 'rewrite').optional(),
    mergeBy: Joi.array().items(Joi.string()).optional()
  }).optional()
})

export const importCsvSchema = Joi.object({
  label: Joi.string(),
  data: Joi.string(),
  options: Joi.object({
    suggestTypes: Joi.boolean().optional(),
    convertNumericValuesToNumbers: Joi.boolean().optional(),
    capitalizeLabels: Joi.boolean().optional(),
    relationshipType: Joi.string().optional(),
    returnResult: Joi.boolean().optional(),
    mergeStrategy: Joi.string().valid('append', 'rewrite').optional(),
    mergeBy: Joi.array().items(Joi.string()).optional()
  }).optional(),
  parseConfig: Joi.object({
    delimiter: Joi.string().max(5).optional(),
    header: Joi.boolean().optional(),
    skipEmptyLines: Joi.alternatives().try(Joi.boolean(), Joi.string().valid('greedy')).optional(),
    dynamicTyping: Joi.boolean().optional(),
    quoteChar: Joi.string().max(3).optional(),
    escapeChar: Joi.string().max(3).optional(),
    newline: Joi.string().max(4).optional()
  }).optional(),
  vectors: Joi.array()
    .items(
      Joi.array().items(
        Joi.object({
          propertyName: Joi.string().required(),
          vector: Joi.array().items(Joi.number()).required(),
          similarityFunction: Joi.string().valid('cosine', 'euclidean').optional()
        })
      )
    )
    .optional()
})
