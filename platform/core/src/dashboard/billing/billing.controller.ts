import {
  Body,
  Controller,
  Get,
  HttpCode,
  HttpStatus,
  Post,
  Query,
  Req,
  ServiceUnavailableException,
  UsePipes,
  UseInterceptors
} from '@nestjs/common'
import { ApiBearerAuth } from '@nestjs/swagger'

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { ValidationPipe } from '@/common/validation/validation.pipe'
import { BillingClientService } from '@/core/billing-client/billing-client.service'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { billingInquirySchema, BillingInquiryBody } from '@/dashboard/billing/billing-inquiry.schema'
import { ChangeCorsInterceptor } from '@/dashboard/common/interceptors/change-cors.interceptor'
import { DataInterceptor } from '@/database/interceptors/data.interceptor'

interface CreateSessionDto {
  priceId: string
  returnUrl: string
  projectId?: string
  discountCode?: string
}

interface CreatePortalSessionDto {
  returnUrl: string
}

interface GetKuHistoryQuery {
  limit?: string
  before?: string
  since?: string
  projectId?: string
  operation?: string
}

@Controller('billing/payment')
@UseInterceptors(TransformResponseInterceptor, NotFoundInterceptor, DataInterceptor, ChangeCorsInterceptor)
export class BillingController {
  constructor(private readonly billingClientService: BillingClientService) {}

  /**
   * Create a Stripe checkout session for subscription upgrade.
   * Proxies request to billing service which handles Stripe integration.
   *
   * POST /api/v1/billing/payment/create-session
   */
  @ApiBearerAuth()
  @AuthGuard('workspace', 'owner')
  @Post('create-session')
  @HttpCode(HttpStatus.CREATED)
  async createCheckoutSession(@Req() request: any, @Body() body: CreateSessionDto) {
    try {
      const { priceId, returnUrl, projectId } = body
      const workspaceId = request.workspaceId

      if (!workspaceId) {
        throw new ServiceUnavailableException('Workspace ID not found in request')
      }

      const session = await this.billingClientService.createCheckoutSession(workspaceId, {
        priceId,
        returnUrl,
        projectId
      })

      return session
    } catch (error: any) {
      throw new ServiceUnavailableException(`Failed to create checkout session: ${error.message}`)
    }
  }

  /**
   * Create a Stripe billing portal session for subscription management.
   * Proxies request to billing service which handles Stripe integration.
   *
   * POST /api/v1/billing/payment/create-portal-session
   */
  @ApiBearerAuth()
  @AuthGuard('workspace', 'owner')
  @Post('create-portal-session')
  @HttpCode(HttpStatus.OK)
  async createPortalSession(@Req() request: any, @Body() body: CreatePortalSessionDto) {
    try {
      const { returnUrl } = body
      const workspaceId = request.workspaceId

      if (!workspaceId) {
        throw new ServiceUnavailableException('Workspace ID not found in request')
      }

      const portalSession = await this.billingClientService.createPortalSession(workspaceId, returnUrl)

      return portalSession
    } catch (error: any) {
      throw new ServiceUnavailableException(`Failed to create portal session: ${error.message}`)
    }
  }

  /**
   * Get current KU usage summary for the authenticated workspace.
   * Used by the dashboard billing page to render the progress bar.
   *
   * GET /api/v1/billing/payment/usage
   */
  @ApiBearerAuth()
  @AuthGuard('workspace')
  @Get('usage')
  @HttpCode(HttpStatus.OK)
  async getUsage(@Req() request: any) {
    try {
      const workspaceId = request.workspaceId

      if (!workspaceId) {
        throw new ServiceUnavailableException('Workspace ID not found in request')
      }

      const usage = await this.billingClientService.getUsage(workspaceId)

      // Billing service disabled (self-hosted / OSS) — return a null-safe default
      if (!usage) {
        return {
          plan: 'self-hosted',
          kuConsumed: 0,
          kuLimit: null,
          kuIncluded: null,
          remaining: null,
          billingModel: 'fixed',
          billingPeriodStart: new Date().toISOString()
        }
      }

      return usage
    } catch (error: any) {
      throw new ServiceUnavailableException(`Failed to get usage: ${error.message}`)
    }
  }

  /**
   * Get KU event history for the current workspace.
   * Returns paginated usage data for displaying in the dashboard.
   *
   * GET /api/v1/billing/payment/ku-history?limit=50&before=2024-01-01T00:00:00Z
   */
  @ApiBearerAuth()
  @AuthGuard('workspace')
  @Get('ku-history')
  @HttpCode(HttpStatus.OK)
  async getKuHistory(@Req() request: any, @Query() query: GetKuHistoryQuery) {
    try {
      const workspaceId = request.workspaceId

      if (!workspaceId) {
        throw new ServiceUnavailableException('Workspace ID not found in request')
      }

      const options: {
        limit?: number
        before?: string
        since?: string
        projectId?: string
        operation?: string
      } = {}
      if (query.limit) {
        options.limit = parseInt(query.limit, 10)
      }
      if (query.before) {
        options.before = query.before
      }
      if (query.since) {
        options.since = query.since
      }
      if (query.projectId) {
        options.projectId = query.projectId
      }
      if (query.operation) {
        options.operation = query.operation
      }

      const history = await this.billingClientService.getKuHistory(workspaceId, options)

      return history || { events: [], hasMore: false, nextCursor: null }
    } catch (error: any) {
      throw new ServiceUnavailableException(`Failed to get KU history: ${error.message}`)
    }
  }

  /**
   * Submit an inquiry for a custom billing plan.
   * Sends an email to the configured admin recipient.
   *
   * POST /api/v1/billing/payment/inquiry
   */
  @ApiBearerAuth()
  @AuthGuard('workspace')
  @Post('inquiry')
  @HttpCode(HttpStatus.OK)
  @UsePipes(ValidationPipe(billingInquirySchema, 'body'))
  async submitInquiry(@Req() request: any, @Body() body: BillingInquiryBody) {
    const workspaceId = request.workspaceId
    const user = request.user

    if (!workspaceId) {
      throw new ServiceUnavailableException('Workspace ID not found in request')
    }

    try {
      await this.billingClientService.submitBillingInquiry({
        contactEmail: body.email.trim(),
        currentPlan: body.currentPlan?.trim() || undefined,
        message: body.message?.trim() || undefined,
        requesterEmail: user?.login,
        workspaceId,
        workspaceName: body.workspaceName?.trim() || undefined
      })

      return { success: true }
    } catch (error: any) {
      throw new ServiceUnavailableException(`Failed to submit billing inquiry: ${error.message}`)
    }
  }
}
