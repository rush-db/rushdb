import { isDevMode } from '@/common/utils/isDevMode'
import { TPlan } from '@/dashboard/billing/stripe/interfaces/stripe.types'

// @TODO: Move to env
export const PRODUCT_PLAN_MAP: TPlan =
  isDevMode() ?
    {
      pro: {
        month: {
          amount: 11,
          priceId: 'price_1PhomaJKGwDMYurpuo4gmg7B',
          productId: 'prod_QYwfBSZk27uH2n'
        },
        annual: {
          amount: 99,
          priceId: 'price_1PhokbJKGwDMYurpab9nLnDX',
          productId: 'prod_QYwd1G0GI2cUIa'
        }
      }
    }
  : {
      pro: {
        month: {
          amount: 11,
          priceId: 'price_1OGYIEJKGwDMYurpDcJtNLCq',
          productId: 'prod_P4hhGEK0PYpRuQ'
        },
        annual: {
          amount: 99,
          priceId: 'price_1OGjWtJKGwDMYurpsE11shVk',
          productId: 'prod_P4tJQJyzJWMI7e'
        }
      }
    }

export enum EConfigKeyByPlan {
  pro = '_PRO'
}
