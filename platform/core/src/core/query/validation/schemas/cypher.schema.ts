import * as Joi from 'joi'

export const cypherSchema = Joi.object({
  query: Joi.string().min(1).required(),
  params: Joi.object().optional()
})
