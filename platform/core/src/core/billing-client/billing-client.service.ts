import { Injectable, Logger } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { HttpService } from '@nestjs/axios'
import { firstValueFrom } from 'rxjs'

import { toBoolean } from '@/common/utils/toBolean'
import { CheckLimitsResponse, Customer, UsageResponse } from './billing-client.types'

/**
 * BillingClientService — communicates with the external billing service API.
 *
 * This service decouples the platform from billing logic. All usage limits,
 * plan enforcement, and subscription state lives in the billing service—not
 * in the platform database.
 *
 * The platform only stores operational data (workspaces, projects, records).
 * It asks the billing service "can I proceed?" before heavy operations.
 */
@Injectable()
export class BillingClientService {
  private readonly logger = new Logger(BillingClientService.name)
  private readonly billingUrl: string
  private readonly secret: string
  private readonly selfHosted: boolean
  private readonly enabled: boolean

  constructor(
    private readonly configService: ConfigService,
    private readonly httpService: HttpService
  ) {
    this.billingUrl = this.configService.get('BILLING_SERVICE_URL') || ''
    this.secret = this.configService.get('RUSHDB_BILLING_SECRET') || ''
    this.selfHosted = toBoolean(this.configService.get('RUSHDB_SELF_HOSTED'))
    this.enabled = !this.selfHosted && !!this.billingUrl
  }

  /**
   * Check if a workspace can proceed with an operation based on limits.
   *
   * @param workspaceId - Workspace to check
   * @param options - Optional check parameters
   * @param options.estimatedKu - Pre-flight KU estimate for the operation
   * @param options.projectCount - Current number of projects
   * @param options.userCount - Current number of users
   * @returns Limit check result with usage stats and operational limits
   */
  async checkLimits(
    workspaceId: string,
    options?: { estimatedKu?: number; projectCount?: number; userCount?: number }
  ): Promise<CheckLimitsResponse> {
    // Self-hosted mode: always allow
    if (this.selfHosted) {
      return {
        allowed: true,
        usage: { kuConsumed: 0, kuLimit: null, plan: 'self-hosted', remaining: null },
        limits: { projectLimit: null, userLimit: null }
      }
    }

    // No billing service configured: allow (pure OSS mode)
    if (!this.enabled) {
      this.logger.warn('Billing service not configured (BILLING_SERVICE_URL not set), allowing operation')
      return {
        allowed: true,
        usage: { kuConsumed: 0, kuLimit: null, plan: 'unknown', remaining: null },
        limits: { projectLimit: null, userLimit: null }
      }
    }

    try {
      const response = await firstValueFrom(
        this.httpService.post<CheckLimitsResponse>(
          `${this.billingUrl}/api/check-limits`,
          { workspaceId, ...options },
          {
            headers: this.secret ? { 'x-rushdb-billing-secret': this.secret } : {},
            timeout: 5000,
            // Don't let axios throw on 4xx — we want to read the response body
            // (e.g. 402 with allowed:false) rather than fall into the catch block.
            validateStatus: () => true
          }
        )
      )

      return response.data
    } catch (error: any) {
      // Only reach here on real network/connectivity failures (ECONNREFUSED, timeout, etc.)
      // — not on HTTP 4xx/5xx responses, because validateStatus:()=>true prevents axios
      // from throwing those. Fail open only for true infrastructure outages.
      this.logger.error(
        `Billing service unreachable for workspace ${workspaceId}: ${error.message} — failing open`
      )

      return {
        allowed: true,
        usage: { kuConsumed: 0, kuLimit: null, plan: 'error', remaining: null },
        limits: { projectLimit: null, userLimit: null }
      }
    }
  }

  /**
   * Create a new customer record in the billing service.
   * Called when a new workspace is created.
   *
   * @param workspaceId - Workspace ID from platform
   * @param plan - Initial plan tier (defaults to 'free')
   * @param ownerEmail - Workspace owner email for billing notifications
   */
  async createCustomer(
    workspaceId: string,
    plan: string = 'free',
    ownerEmail?: string | null
  ): Promise<Customer | null> {
    if (!this.enabled) {
      this.logger.debug('Billing service not configured, skipping customer creation')
      return null
    }

    try {
      const response = await firstValueFrom(
        this.httpService.post<{ ok: boolean; customer: Customer }>(
          `${this.billingUrl}/api/customers`,
          { workspaceId, plan, ownerEmail: ownerEmail ?? null },
          {
            headers: this.secret ? { 'x-rushdb-billing-secret': this.secret } : {},
            timeout: 5000
          }
        )
      )

      this.logger.log(`Created billing customer for workspace ${workspaceId} (plan: ${plan})`)
      return response.data.customer
    } catch (error: any) {
      // Don't block workspace creation if billing service fails
      this.logger.error(`Failed to create customer for workspace ${workspaceId}: ${error.message}`)
      return null
    }
  }

  /**
   * Get customer details from billing service.
   *
   * @param workspaceId - Workspace ID
   * @returns Customer record or null if not found
   */
  async getCustomer(workspaceId: string): Promise<Customer | null> {
    if (!this.enabled) return null

    try {
      const response = await firstValueFrom(
        this.httpService.get<Customer>(`${this.billingUrl}/api/customers/${workspaceId}`, {
          headers: this.secret ? { 'x-rushdb-billing-secret': this.secret } : {},
          timeout: 5000
        })
      )

      return response.data
    } catch (error: any) {
      if (error.response?.status === 404) {
        return null
      }

      this.logger.error(`Failed to get customer for workspace ${workspaceId}: ${error.message}`)
      return null
    }
  }

  /**
   * Update customer details in billing service.
   *
   * @param workspaceId - Workspace ID
   * @param updates - Partial customer updates
   * @returns Updated customer or null if failed
   */
  async updateCustomer(workspaceId: string, updates: Partial<Customer>): Promise<Customer | null> {
    if (!this.enabled) return null

    try {
      const response = await firstValueFrom(
        this.httpService.patch<{ ok: boolean; customer: Customer }>(
          `${this.billingUrl}/api/customers/${workspaceId}`,
          updates,
          {
            headers: this.secret ? { 'x-rushdb-billing-secret': this.secret } : {},
            timeout: 5000
          }
        )
      )

      return response.data.customer
    } catch (error: any) {
      this.logger.error(`Failed to update customer for workspace ${workspaceId}: ${error.message}`)
      return null
    }
  }

  /**
   * Delete customer record from billing service.
   * Called when a workspace is deleted.
   *
   * @param workspaceId - Workspace ID
   */
  async deleteCustomer(workspaceId: string): Promise<boolean> {
    if (!this.enabled) return true

    try {
      await firstValueFrom(
        this.httpService.delete(`${this.billingUrl}/api/customers/${workspaceId}`, {
          headers: this.secret ? { 'x-rushdb-billing-secret': this.secret } : {},
          timeout: 5000
        })
      )

      this.logger.log(`Deleted billing customer for workspace ${workspaceId}`)
      return true
    } catch (error: any) {
      this.logger.error(`Failed to delete customer for workspace ${workspaceId}: ${error.message}`)
      return false
    }
  }

  /**
   * Create a Stripe checkout session for subscription upgrades.
   * Proxies the request to the billing service.
   *
   * @param workspaceId - Workspace ID
   * @param dto - Checkout session creation params
   * @returns Session ID for Stripe redirect
   */
  async createCheckoutSession(
    workspaceId: string,
    dto: { priceId: string; returnUrl: string; projectId?: string }
  ): Promise<{ id: string; url: string } | null> {
    if (!this.enabled) {
      throw new Error('Billing service not available in self-hosted mode')
    }

    try {
      const response = await firstValueFrom(
        this.httpService.post<{ id: string; url: string }>(
          `${this.billingUrl}/api/checkout-session`,
          { workspaceId, ...dto },
          {
            headers: this.secret ? { 'x-rushdb-billing-secret': this.secret } : {},
            timeout: 10000
          }
        )
      )

      return response.data
    } catch (error: any) {
      this.logger.error(`Failed to create checkout session for workspace ${workspaceId}: ${error.message}`)
      throw error
    }
  }

  /**
   * Create a Stripe billing portal session for subscription management.
   * Proxies the request to the billing service.
   *
   * @param workspaceId - Workspace ID
   * @param returnUrl - URL to return to after portal session
   * @returns Portal redirect URL
   */
  async createPortalSession(workspaceId: string, returnUrl: string): Promise<{ redirectUrl: string } | null> {
    if (!this.enabled) {
      throw new Error('Billing service not available in self-hosted mode')
    }

    try {
      const response = await firstValueFrom(
        this.httpService.post<{ redirectUrl: string }>(
          `${this.billingUrl}/api/portal-session`,
          { workspaceId, returnUrl },
          {
            headers: this.secret ? { 'x-rushdb-billing-secret': this.secret } : {},
            timeout: 10000
          }
        )
      )

      return response.data
    } catch (error: any) {
      this.logger.error(`Failed to create portal session for workspace ${workspaceId}: ${error.message}`)
      throw error
    }
  }

  /**
   * Get current KU usage summary for a workspace.
   * Used by the dashboard billing page to render the progress bar.
   *
   * @param workspaceId - Workspace ID
   * @returns Usage summary or null when billing service is disabled
   */
  async getUsage(workspaceId: string): Promise<UsageResponse | null> {
    if (!this.enabled) return null

    try {
      const response = await firstValueFrom(
        this.httpService.get<UsageResponse>(`${this.billingUrl}/api/customers/${workspaceId}/usage`, {
          headers: this.secret ? { 'x-rushdb-billing-secret': this.secret } : {},
          timeout: 5000
        })
      )

      return response.data
    } catch (error: any) {
      this.logger.error(`Failed to get usage for workspace ${workspaceId}: ${error.message}`)
      return null
    }
  }

  /**
   * Get KU event history for a workspace.
   * Used for displaying usage analytics in the dashboard.
   *
   * @param workspaceId - Workspace ID
   * @param options - Pagination options
   * @param options.limit - Number of events to fetch (max 100)
   * @param options.before - ISO timestamp cursor for pagination
   * @returns Paginated KU events
   */
  async getKuHistory(
    workspaceId: string,
    options?: { limit?: number; before?: string; since?: string; projectId?: string; operation?: string }
  ): Promise<{
    events: Array<{
      id: string
      workspaceId: string
      projectId: string
      operation: string
      kuConsumed: number
      metadata: Record<string, unknown> | null
      timestamp: string
    }>
    hasMore: boolean
    nextCursor: string | null
  } | null> {
    if (!this.enabled) return null

    try {
      const params = new URLSearchParams({
        workspaceId,
        ...(options?.limit && { limit: options.limit.toString() }),
        ...(options?.before && { before: options.before }),
        ...(options?.since && { since: options.since }),
        ...(options?.projectId && { projectId: options.projectId }),
        ...(options?.operation && { operation: options.operation })
      })

      const response = await firstValueFrom(
        this.httpService.get(`${this.billingUrl}/api/ku-events/history?${params}`, {
          headers: this.secret ? { 'x-rushdb-billing-secret': this.secret } : {},
          timeout: 10000
        })
      )

      return response.data
    } catch (error: any) {
      this.logger.error(`Failed to get KU history for workspace ${workspaceId}: ${error.message}`)
      return null
    }
  }
}
