import Joi = require('joi')

import { SORT_DESC, SORT_ASC } from '@/core/search/search.constants'

// Recursive expression schema for the select clause.
// Uses Joi.link to handle the recursive Expr type.
const exprSchema: Joi.Schema = Joi.alternatives()
  .try(
    Joi.string(), // field ref "$record.field" or literal string
    Joi.number(), // literal number
    Joi.boolean(), // literal boolean
    Joi.object({ $ref: Joi.string().required() }),
    Joi.object({ $sum: Joi.link('#expr').required() }),
    Joi.object({
      $avg: Joi.link('#expr').required(),
      $precision: Joi.number().integer().min(0).optional()
    }),
    Joi.object({
      $count: Joi.alternatives().try(Joi.string().valid('*'), Joi.link('#expr')).required()
    }),
    Joi.object({ $min: Joi.link('#expr').required() }),
    Joi.object({ $max: Joi.link('#expr').required() }),
    Joi.object({ $divide: Joi.array().items(Joi.link('#expr')).length(2).required() }),
    Joi.object({ $multiply: Joi.array().items(Joi.link('#expr')).length(2).required() }),
    Joi.object({ $add: Joi.array().items(Joi.link('#expr')).length(2).required() }),
    Joi.object({ $subtract: Joi.array().items(Joi.link('#expr')).length(2).required() }),
    Joi.object({ $collect: Joi.object().required() }),
    Joi.object({ $timeBucket: Joi.object().required() })
  )
  .id('expr')
  .meta({ className: 'Expr' })

const selectSchema = Joi.object().pattern(Joi.string(), exprSchema)

const searchDtoSchema = Joi.object({
  limit: Joi.number().min(1).max(1000).optional(),
  skip: Joi.number().min(0).optional(),
  orderBy: Joi.alternatives().try(
    Joi.string().valid(SORT_ASC, SORT_DESC),
    Joi.object().pattern(Joi.string(), Joi.string().valid(SORT_ASC, SORT_DESC)).optional()
  ),
  labels: Joi.array().items(Joi.string().allow(null)).optional(),
  where: Joi.object(),
  select: selectSchema.optional(),
  aggregate: Joi.object().optional() // legacy — loose validation preserved
})

export const searchSchema = searchDtoSchema
