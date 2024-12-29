import Joi = require('joi')

import { SORT_DESC, SORT_ASC } from '@/core/search/search.constants'

const searchDtoSchema = Joi.object({
  limit: Joi.number().min(1).max(1000).optional(),
  skip: Joi.number().min(0).optional(),
  orderBy: Joi.alternatives().try(
    Joi.string().valid(SORT_ASC, SORT_DESC),
    Joi.object().pattern(Joi.string(), Joi.string().valid(SORT_ASC, SORT_DESC)).optional()
  ),
  labels: Joi.array().items(Joi.string().allow(null)).optional(),
  where: Joi.object()
})

export const searchSchema = searchDtoSchema
