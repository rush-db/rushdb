import Joi = require('joi')

import { RUSHDB_LABEL_RECORD } from '@/core/common/constants'

export const importJsonSchema = Joi.object({
  label: Joi.string(),
  payload: Joi.alternatives().try(Joi.object(), Joi.array().items(Joi.object())),
  options: Joi.object({
    suggestTypes: Joi.boolean().optional()
  }).optional()
})

export const importCsvSchema = Joi.object({
  label: Joi.string(),
  payload: Joi.string(),
  options: Joi.object({
    suggestTypes: Joi.boolean().optional()
  }).optional()
})
