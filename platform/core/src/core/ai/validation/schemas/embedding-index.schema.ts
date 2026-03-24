import Joi = require('joi')

export const createEmbeddingIndexSchema = Joi.object({
  label: Joi.string().required().min(1).message('label is required and must be a non-empty string'),
  propertyName: Joi.string()
    .required()
    .min(1)
    .message('propertyName is required and must be a non-empty string')
})
