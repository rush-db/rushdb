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
  // Two allowed shapes:
  // 1) Key-based join: both source.key and target.key provided
  // 2) Many-to-many: keys omitted but manyToMany=true and both sides must provide non-empty `where` filters
  source: Joi.alternatives().try(
    Joi.object({
      label: Joi.string().required().not(''),
      key: Joi.string().required().not(''),
      where: Joi.object().optional().unknown(true)
    }).required(),
    Joi.object({
      label: Joi.string().required().not(''),
      key: Joi.string().optional().allow('', null),
      where: Joi.object().min(1).required().unknown(true)
    }).required()
  ),
  target: Joi.alternatives().try(
    Joi.object({
      label: Joi.string().required().not(''),
      key: Joi.string().required().not(''),
      where: Joi.object().optional().unknown(true)
    }).required(),
    Joi.object({
      label: Joi.string().required().not(''),
      key: Joi.string().optional().allow('', null),
      where: Joi.object().min(1).required().unknown(true)
    }).required()
  ),
  type: Joi.string().optional().not(''),
  direction: Joi.string().valid('in', 'out', 'IN', 'OUT').optional(),
  manyToMany: Joi.boolean().optional()
})
