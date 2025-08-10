import {
  Body,
  Controller,
  HttpCode,
  HttpStatus,
  Post,
  Req,
  Res,
  ServiceUnavailableException,
  UseInterceptors
} from '@nestjs/common'
import { ApiBearerAuth, ApiExcludeController } from '@nestjs/swagger'
import { FastifyReply, FastifyRequest } from 'fastify'
import { Transaction } from 'neo4j-driver'

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { PlansDto } from '@/dashboard/billing/stripe/plans.dto'
import { ChangeCorsInterceptor } from '@/dashboard/common/interceptors/change-cors.interceptor'
import { AuthUser } from '@/dashboard/user/decorators/user.decorator'
import { IUserClaims } from '@/dashboard/user/interfaces/user-claims.interface'
import { NeogmaDataInterceptor } from '@/database/neogma/neogma-data.interceptor'
import { TransactionDecorator } from '@/database/neogma/transaction.decorator'

import { StripeService } from './stripe.service'

@Controller('billing')
@ApiExcludeController()
@UseInterceptors(
  TransformResponseInterceptor,
  NotFoundInterceptor,
  NeogmaDataInterceptor,

  ChangeCorsInterceptor
)
export class StripeController {
  constructor(private paymentService: StripeService) {}

  @ApiBearerAuth()
  @AuthGuard('workspace', 'owner')
  @Post('payment/create-session')
  async createCheckoutSession(
    @AuthUser() user: IUserClaims,
    @Body() plansDto: PlansDto,
    @Res() response: FastifyReply
  ) {
    try {
      const session = await this.paymentService.createCheckoutSession(plansDto, user.login)
      response.status(HttpStatus.CREATED).send(session)
    } catch (error) {
      response.status(HttpStatus.INTERNAL_SERVER_ERROR).send(error.message)
    }
  }

  @ApiBearerAuth()
  @AuthGuard('workspace', 'owner')
  @Post('payment/create-portal-session')
  @HttpCode(HttpStatus.OK)
  async createPortalSession(@AuthUser() user: IUserClaims, @Body() config: Record<string, string>) {
    const { returnUrn } = config

    try {
      const portalSession = await this.paymentService.createPortalSession(user.login, returnUrn)

      return {
        redirectUrl: portalSession.url
      }
    } catch (error) {
      throw new ServiceUnavailableException(`Can't process new portal session: ${error.message}`)
    }
  }

  @Post('payment/webhook')
  async handleStripeWebhook(
    @Req() request: FastifyRequest,
    @Res() response: FastifyReply,
    @TransactionDecorator() transaction: Transaction
  ) {
    const signature = request.headers['stripe-signature']
    const rawBody = request['rawBody']

    if (!Buffer.isBuffer(rawBody) && typeof rawBody !== 'string') {
      return response.status(HttpStatus.BAD_REQUEST).send('Invalid request body')
    }
    try {
      const event = this.paymentService.constructEvent(rawBody, signature)

      switch (event.type) {
        case 'customer.subscription.created':
          await this.paymentService.createCustomerPlan(event, transaction)
          return response.status(HttpStatus.OK).send({ received: true })
        case 'customer.subscription.updated':
          await this.paymentService.updateCustomerPlan(event, transaction)
          return response.status(HttpStatus.OK).send({ received: true })
        case 'checkout.session.completed':
          await this.paymentService.updateCustomerIfNeeded(event)
          await this.paymentService.createCustomerPlan(event, transaction)
          return response.status(HttpStatus.OK).send({ received: true })
        case 'customer.subscription.deleted':
          await this.paymentService.deleteCustomerPlan(event, transaction)
          return response.status(HttpStatus.OK).send({ received: true })
      }

      response.status(HttpStatus.OK).send({ received: true })
      return
    } catch (err) {
      response.status(HttpStatus.BAD_REQUEST).send(`Webhook Error: ${err.message}`)
      return
    }
  }
}
