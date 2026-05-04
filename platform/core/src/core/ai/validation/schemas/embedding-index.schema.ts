// eslint-disable-next-line @typescript-eslint/no-require-imports
import Joi = require('joi')

export const createEmbeddingIndexSchema = Joi.object({
  label: Joi.string().required().min(1).message('label is required and must be a non-empty string'),
  propertyName: Joi.string()
    .required()
    .min(1)
    .message('propertyName is required and must be a non-empty string'),
  sourceType: Joi.string().valid('managed', 'external').optional(),
  external: Joi.boolean().optional(),
  similarityFunction: Joi.string().valid('cosine', 'euclidean').optional(),
  dimensions: Joi.number().integer().min(1).max(4096).optional()
})
