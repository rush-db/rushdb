import { forwardRef, HttpException, HttpStatus, Inject, Injectable } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import axios, { AxiosRequestConfig } from 'axios'
import { Transaction } from 'neo4j-driver'
import Stripe from 'stripe'

import { isProductionMode } from '@/common/utils/isProductionMode'
import { EConfigKeyByPlan } from '@/dashboard/billing/stripe/interfaces/stripe.constans'
import { PlansData } from '@/dashboard/billing/stripe/interfaces/stripe.types'
import { PlansDto } from '@/dashboard/billing/stripe/plans.dto'
import { ProjectService } from '@/dashboard/project/project.service'
import { WorkspaceService } from '@/dashboard/workspace/workspace.service'

@Injectable()
export class StripeService {
  private stripe

  constructor(
    private readonly configService: ConfigService,
    @Inject(forwardRef(() => WorkspaceService))
    private readonly workspaceService: WorkspaceService,
    @Inject(forwardRef(() => ProjectService))
    private readonly projectService: ProjectService
  ) {
    this.stripe = new Stripe(this.configService.get('STRIPE_SECRET_KEY'), {
      apiVersion: '2023-10-16'
    })
  }

  async createCustomer(email: string): Promise<Stripe.Customer> {
    return this.stripe.customers.create({
      email: email
    })
  }

  async updateCustomerIfNeeded(payload: Stripe.Event) {
    const session = payload.data.object as Stripe.Checkout.Session
    const customerId = typeof session.customer === 'string' ? session.customer : session.customer.id

    const customer = await this.stripe.customers.retrieve(customerId)

    const updatePayload = {
      name: session.customer_details.name,
      address: session.customer_details.address
    }

    if (customer.name !== updatePayload.name || customer.address !== updatePayload.address) {
      await this.updateCustomerDetails(customerId, updatePayload)
    }
  }

  async updateCustomerDetails(customerId: string, payload: { name?: string; address?: Stripe.Address }) {
    const updates: Stripe.CustomerUpdateParams = {}

    if (payload.name) {
      updates.name = payload.name
    }

    if (payload.address) {
      updates.address = payload.address
    }

    await this.stripe.customers.update(customerId, updates)
  }

  async getPrices() {
    const { data } = await axios.get<PlansData>('https://billing.rushdb.com/api/prices', {
      ...(!isProductionMode() && { headers: { 'x-env-id': 'dev' } })
    } as AxiosRequestConfig)

    return data
  }

  async createCustomerPlan(payload: Stripe.Event, transaction: Transaction) {
    const stripePayload = payload.data.object

    const isCheckoutSession = 'object' in stripePayload && stripePayload.object === 'checkout.session'
    const isSubscription = 'object' in stripePayload && stripePayload.object === 'subscription'

    let subscriptionId: string
    let userEmail: string
    let projectId: string | undefined

    if (isCheckoutSession) {
      const session = stripePayload as Stripe.Checkout.Session
      subscriptionId = session.subscription as string
      userEmail = session.customer_email || (await this.stripe.customers.retrieve(session.customer)).email
      projectId = session.metadata?.projectId ?? session.client_reference_id ?? undefined
    } else if (isSubscription) {
      const subscription = stripePayload as Stripe.Subscription
      subscriptionId = subscription.id
      projectId = subscription.metadata?.projectId

      const customer = await this.stripe.customers.retrieve(subscription.customer as string)
      userEmail = customer.email
    } else {
      throw new HttpException('Unsupported event payload.', HttpStatus.BAD_REQUEST)
    }

    if (subscriptionId && userEmail) {
      const subscription = await this.stripe.subscriptions.retrieve(subscriptionId)
      const subscriptionPriceId = subscription.items.data[0].plan.id
      const subscriptionProductId = subscription.items.data[0].product
      const validTill = new Date(subscription.current_period_end * 1000)
      const subscriptionStatus = subscription.status

      const workspaceId = await this.workspaceService.findUserBillingWorkspace(userEmail, transaction)
      // team/pro
      const planLevel = await this.getPlanKeyByPriceId(subscriptionPriceId)

      if (!projectId) {
        await this.updateWorkspaceSubscription(
          {
            subscriptionStatus,
            validTill,
            targetId: workspaceId,
            planId: planLevel,
            subscriptionPriceId,
            subscriptionProductId,
            payload
          },
          transaction
        )
      } else {
        // @TODO: Check if there is no other active subscriptions (other projects/or workspace-level subscriptions)
        await this.updateWorkspaceSubscription(
          {
            subscriptionStatus,
            validTill,
            targetId: workspaceId,
            planId: planLevel,
            subscriptionPriceId,
            subscriptionProductId,
            payload
          },
          transaction
        )
        await this.updateProjectSubscription(
          {
            subscriptionStatus,
            validTill,
            targetId: projectId,
            planId: planLevel,
            subscriptionPriceId,
            subscriptionProductId,
            payload
          },
          transaction
        )
      }
    }
  }

  async updateWorkspaceSubscription(
    {
      subscriptionStatus,
      subscriptionPriceId,
      subscriptionProductId,
      payload,
      validTill,
      targetId,
      planId
    }: {
      subscriptionStatus: string
      validTill: Date
      targetId: string
      planId: string
      payload: Stripe.Event
      subscriptionPriceId: string
      subscriptionProductId: string
    },
    transaction: Transaction
  ) {
    if ('cancel_at_period_end' in payload.data.object && payload.data.object.cancel_at_period_end === true) {
      await this.workspaceService.patchWorkspace(
        targetId,
        {
          planId,
          isSubscriptionCancelled: true
        },
        transaction
      )
    } else if (subscriptionStatus === 'active') {
      await this.workspaceService.patchWorkspace(
        targetId,
        {
          isSubscriptionCancelled: false,
          planId,
          validTill: validTill.toISOString(),
          subscriptionPriceId,
          subscriptionProductId,
          limits: JSON.stringify(this.workspaceService.getLimitsByKey(EConfigKeyByPlan[planId]))
        },
        transaction
      )
    }
  }

  async updateProjectSubscription(
    {
      subscriptionStatus,
      subscriptionPriceId,
      subscriptionProductId,
      payload,
      validTill,
      targetId,
      planId
    }: {
      subscriptionStatus: string
      validTill: Date
      targetId: string
      planId: string
      payload: Stripe.Event
      subscriptionPriceId: string
      subscriptionProductId: string
    },
    transaction: Transaction
  ) {
    if ('cancel_at_period_end' in payload.data.object && payload.data.object.cancel_at_period_end === true) {
      await this.projectService.updateProject(
        targetId,
        {
          planId,
          isSubscriptionCancelled: true
        },
        transaction
      )
    } else if (subscriptionStatus === 'active') {
      await this.projectService.updateProject(
        targetId,
        {
          isSubscriptionCancelled: false,
          planId,
          subscriptionPriceId,
          subscriptionProductId,
          status: 'provisioning',
          validTill: validTill.toISOString()
        },
        transaction
      )
      await this.projectService.notifyRushDBAdmin(targetId, transaction)
    }
  }

  async getPlanKeyByPriceId(subscriptionPriceId: string): Promise<keyof PlansData | null> {
    const plansData = await this.getPrices()

    const entries = Object.entries(plansData) as [keyof PlansData, PlansData[keyof PlansData]][]

    for (const [planKey, planValue] of entries) {
      // team is an array of tier objects with onDemand/reserved
      if (Array.isArray(planValue)) {
        for (const tier of planValue) {
          if (!tier || typeof tier !== 'object') {
            continue
          }
          if (tier.onDemand && tier.onDemand.priceId === subscriptionPriceId) {
            return planKey
          }
          if (tier.reserved && tier.reserved.priceId === subscriptionPriceId) {
            return planKey
          }
        }
      } else if (planValue && typeof planValue === 'object') {
        // pro is an object with monthly and annual entries (each has priceId)
        const periods = planValue as { [key: string]: { priceId?: string } }
        for (const period of Object.values(periods)) {
          if (period && period.priceId === subscriptionPriceId) {
            return planKey
          }
        }
      }
    }

    return null
  }

  async updateCustomerPlan(payload: Stripe.Event, transaction: Transaction) {
    const subscription = payload.data.object as Stripe.Subscription
    const projectId = subscription.metadata?.projectId

    const customer = await this.stripe.customers.retrieve(subscription.customer as string)
    const userEmail = customer.email

    const subscriptionPriceId = subscription.items.data[0].plan.id // price_id
    const subscriptionProductId = subscription.items.data[0].plan.product // product_id
    const subscriptionStatus = subscription.status
    const validTill = new Date(subscription.current_period_end * 1000)

    const workspaceId = await this.workspaceService.findUserBillingWorkspace(userEmail, transaction)
    const planId = await this.getPlanKeyByPriceId(subscriptionPriceId)

    if (!projectId) {
      await this.updateWorkspaceSubscription(
        {
          subscriptionStatus,
          validTill,
          targetId: workspaceId,
          planId,
          subscriptionPriceId,
          subscriptionProductId: subscriptionProductId as string,
          payload
        },
        transaction
      )
    } else {
      // @TODO: Check if there is no other active subscriptions (other projects/or workspace-level subscriptions)
      await this.updateWorkspaceSubscription(
        {
          subscriptionStatus,
          validTill,
          targetId: workspaceId,
          planId,
          subscriptionPriceId,
          subscriptionProductId: subscriptionProductId as string,
          payload
        },
        transaction
      )
      await this.updateProjectSubscription(
        {
          subscriptionStatus,
          validTill,
          targetId: projectId,
          planId,
          subscriptionPriceId,
          subscriptionProductId: subscriptionProductId as string,
          payload
        },
        transaction
      )
    }
  }

  async deleteCustomerPlan(payload: Stripe.Event, transaction: Transaction) {
    const deletedSubscription = payload.data.object as Stripe.Subscription
    const projectId = deletedSubscription.metadata?.projectId

    const customer = await this.stripe.customers.retrieve(deletedSubscription.customer as string)
    const userEmail = customer.email

    const targetId = await this.workspaceService.findUserBillingWorkspace(userEmail, transaction)

    if (!projectId) {
      await this.workspaceService.dropWorkspaceSubscription(targetId, transaction)
    } else {
      // @TODO: Check if there is no other active subscriptions (other projects/or workspace-level subscriptions)
      await this.workspaceService.dropWorkspaceSubscription(targetId, transaction)
      await this.projectService.dropProjectSubscription(projectId, transaction)
    }
  }

  async getCustomerByEmail(email: string): Promise<Stripe.Customer | null> {
    const customers = await this.stripe.customers.list({
      email,
      limit: 1
    })

    if (customers.data.length) {
      return customers.data[0]
    } else {
      return null
    }
  }

  async createCheckoutSession(
    { priceId, projectId, returnUrl }: PlansDto,
    email: string
  ): Promise<Stripe.Checkout.Session> {
    const customer: Stripe.Customer = await this.getCustomerByEmail(email)

    return await this.stripe.checkout.sessions.create({
      customer: customer.id,
      line_items: [{ price: priceId, quantity: 1 }],
      currency: 'usd',
      mode: 'subscription',
      success_url: `${returnUrl}?payment_successful=true`,
      cancel_url: `${returnUrl}?payment_successful=false`,
      metadata: {
        projectId
      },
      client_reference_id: projectId,
      subscription_data: {
        metadata: {
          projectId
        }
      }
    })
  }

  async createPortalSession(
    email: string,
    returnUrl: string
  ): Promise<Stripe.Response<Stripe.BillingPortal.Session>> {
    const customer: Stripe.Customer = await this.getCustomerByEmail(email)

    return await this.stripe.billingPortal.sessions.create({
      customer: customer.id,
      return_url: returnUrl
    })
  }

  constructEvent(payload: Buffer | string, signature: string | string[]): Stripe.Event {
    const endpointSecret = this.configService.get('STRIPE_ENDPOINT_SECRET')

    return this.stripe.webhooks.constructEvent(payload, signature, endpointSecret)
  }
}
