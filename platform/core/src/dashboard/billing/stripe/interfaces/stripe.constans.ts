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
          amount: 19,
          priceId: 'price_1QfcypJKGwDMYurpc9GTBUzb',
          productId: 'prod_RYkT6hrMw3XBGI'
        },
        annual: {
          amount: 199,
          priceId: 'price_1QfcwoJKGwDMYurplLpyerlu',
          productId: 'prod_RYkRuqwNDrnyr7'
        }
      }
    }

export enum EConfigKeyByPlan {
  pro = '_PRO'
}
