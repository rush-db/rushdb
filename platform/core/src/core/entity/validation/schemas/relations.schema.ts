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
