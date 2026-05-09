import * as Joi from 'joi'

export const billingInquirySchema = Joi.object({
  email: Joi.string().email().max(320).required(),
  message: Joi.string().allow('').max(2000).optional(),
  workspaceName: Joi.string().allow('').max(200).optional(),
  currentPlan: Joi.string().allow('').max(64).optional()
})

export interface BillingInquiryBody {
  email: string
  message?: string
  workspaceName?: string
  currentPlan?: string
}
