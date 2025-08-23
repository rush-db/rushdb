import Joi = require('joi')

export const targetIdsSchema = Joi.alternatives()
  .try(Joi.string().required().not(''), Joi.array().items(Joi.string().required().not('')))
  .required()
  .not('')

export const createRelationSchema = Joi.object({
  targetIds: targetIdsSchema,
  type: Joi.string().optional().not('')
})

export const deleteRelationsSchema = Joi.object({
  targetIds: targetIdsSchema,
  typeOrTypes: Joi.alternatives()
    .try(Joi.string().optional().not(''), Joi.array().items(Joi.string().optional().not('')))
    .optional()
    .not('')
})

export const createRelationsByKeysSchema = Joi.object({
  source: Joi.object({
    label: Joi.string().required().not(''),
    key: Joi.string().required().not(''),
    where: Joi.object().optional().unknown(true)
  }).required(),
  target: Joi.object({
    label: Joi.string().required().not(''),
    key: Joi.string().required().not(''),
    where: Joi.object().optional().unknown(true)
  }).required(),
  type: Joi.string().optional().not(''),
  direction: Joi.string().valid('in', 'out', 'IN', 'OUT').optional()
})
