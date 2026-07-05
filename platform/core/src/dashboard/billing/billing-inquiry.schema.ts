import { z } from 'zod'

export const billingInquirySchema = z
  .object({
    email: z.string().email().max(320),
    message: z.string().max(2000).optional(),
    workspaceName: z.string().max(200).optional(),
    currentPlan: z.string().max(64).optional()
  })
  .passthrough()

export interface BillingInquiryBody {
  email: string
  message?: string
  workspaceName?: string
  currentPlan?: string
}
