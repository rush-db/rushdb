import { forwardRef, Inject, Injectable } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { Transaction } from 'neo4j-driver'
import Stripe from 'stripe'

import { EConfigKeyByPlan } from '@/dashboard/billing/stripe/interfaces/stripe.constans'
import { PlansDto } from '@/dashboard/billing/stripe/plans.dto'
import { getPlanKeyByPriceId } from '@/dashboard/billing/stripe/stripe.utils'
import { WorkspaceService } from '@/dashboard/workspace/workspace.service'
import axios, { AxiosRequestConfig } from 'axios'
import { TPlan } from '@/dashboard/billing/stripe/interfaces/stripe.types'
import { isProductionMode } from '@/common/utils/isProductionMode'

@Injectable()
export class StripeService {
  private stripe

  constructor(
    private readonly configService: ConfigService,
    @Inject(forwardRef(() => WorkspaceService))
    private readonly workspaceService: WorkspaceService
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

  async createCustomerPlan(payload: Stripe.Event, transaction: Transaction) {
    const session = payload.data.object as Stripe.Checkout.Session
    const subscriptionId = session.subscription

    const userEmail = session.customer_email || (await this.stripe.customers.retrieve(session.customer)).email

    if (subscriptionId) {
      const { data } = await axios.get<TPlan>('https://billing.rushdb.com/api/prices', {
        ...(!isProductionMode() && { headers: { 'x-env-id': 'dev' } })
      } as AxiosRequestConfig)

      const subscription = await this.stripe.subscriptions.retrieve(subscriptionId)
      const planId = subscription.items.data[0].plan.id
      const validTill = new Date(subscription.current_period_end * 1000)

      //@TODO: CHECK OWNERSHIP FOR WORKSPACE
      const targetId = await this.workspaceService.findUserWorkspace(userEmail, transaction)
      const plan = getPlanKeyByPriceId(planId, data)

      await this.workspaceService.patchWorkspace(
        targetId,
        {
          planId: plan,
          validTill: validTill.toISOString(),
          isSubscriptionCancelled: false,
          limits: JSON.stringify(this.workspaceService.getLimitsByKey(EConfigKeyByPlan[plan]))
        },
        transaction
      )
    }
  }

  async updateCustomerPlan(payload: Stripe.Event, transaction: Transaction) {
    const updatedSubscription = payload.data.object as Stripe.Subscription

    const { data } = await axios.get<TPlan>('https://billing.rushdb.com/api/prices', {
      ...(!isProductionMode() && { headers: { 'x-env-id': 'dev' } })
    } as AxiosRequestConfig)

    const customer = await this.stripe.customers.retrieve(updatedSubscription.customer as string)
    const userEmail = customer.email

    const updatedPlanId = updatedSubscription.items.data[0].plan.id
    const updateStatus = updatedSubscription.status
    const updatedValidTill = new Date(updatedSubscription.current_period_end * 1000)

    const targetId = await this.workspaceService.findUserWorkspace(userEmail, transaction)
    const plan = getPlanKeyByPriceId(updatedPlanId, data)

    if ('cancel_at_period_end' in payload.data.object && payload.data.object.cancel_at_period_end === true) {
      await this.workspaceService.patchWorkspace(
        targetId,
        {
          planId: plan,
          isSubscriptionCancelled: true
        },
        transaction
      )
    } else if (updateStatus === 'active') {
      await this.workspaceService.patchWorkspace(
        targetId,
        {
          isSubscriptionCancelled: false,
          planId: plan,
          validTill: updatedValidTill.toISOString(),
          limits: JSON.stringify(this.workspaceService.getLimitsByKey(EConfigKeyByPlan[plan]))
        },
        transaction
      )
    }
  }

  async deleteCustomerPlan(payload: Stripe.Event, transaction: Transaction) {
    const deletedSubscription = payload.data.object as Stripe.Subscription
    const customer = await this.stripe.customers.retrieve(deletedSubscription.customer as string)
    const userEmail = customer.email

    const targetId = await this.workspaceService.findUserWorkspace(userEmail, transaction)
    await this.workspaceService.dropWorkspaceSubscription(targetId, transaction)
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
    { id, period, returnUrl }: PlansDto,
    email: string
  ): Promise<Stripe.Checkout.Session> {
    const { data } = await axios.get<TPlan>('https://billing.rushdb.com/api/prices', {
      ...(!isProductionMode() && { headers: { 'x-env-id': 'dev' } })
    } as AxiosRequestConfig)
    const { priceId } = data[id][period]

    const customer: Stripe.Customer = await this.getCustomerByEmail(email)

    return await this.stripe.checkout.sessions.create({
      customer: customer.id,
      line_items: [
        {
          price: priceId,
          quantity: 1
        }
      ],
      currency: 'usd',
      mode: 'subscription',
      success_url: `${returnUrl}?payment_successful=true`,
      cancel_url: `${returnUrl}?payment_successful=false`
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
