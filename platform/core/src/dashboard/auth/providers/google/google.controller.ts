import {
  Controller,
  Get,
  Logger,
  Query,
  Redirect,
  UnauthorizedException,
  UseInterceptors
} from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { ApiExcludeController, ApiTags } from '@nestjs/swagger'
import { Transaction } from 'neo4j-driver'
import * as queryString from 'query-string'

import { CommonResponseDecorator } from '@/common/decorators/common-response.decorator'
import { isDevMode } from '@/common/utils/isDevMode'
import { AuthService } from '@/dashboard/auth/auth.service'
import { IOauthUrl } from '@/dashboard/auth/auth.types'
import { GetOauthDto } from '@/dashboard/auth/dto/get-oauth.dto'
import { EmailConfirmationService } from '@/dashboard/auth/email-confirmation/email-confirmation.service'
import { GoogleOAuthService } from '@/dashboard/auth/providers/google/google.service'
import { ChangeCorsInterceptor } from '@/dashboard/common/interceptors/change-cors.interceptor'
import { GetUserDto } from '@/dashboard/user/dto/get-user.dto'
import { User } from '@/dashboard/user/user.entity'
import { NeogmaDataInterceptor } from '@/database/neogma/neogma-data.interceptor'
import { NeogmaTransactionInterceptor } from '@/database/neogma/neogma-transaction.interceptor'
import { TransactionDecorator } from '@/database/neogma/transaction.decorator'

@Controller('auth')
@ApiExcludeController()
export class GoogleOAuthController {
  constructor(
    private readonly authService: AuthService,
    private readonly googleOAuthService: GoogleOAuthService,
    private readonly configService: ConfigService,
    private readonly emailConfirmationService: EmailConfirmationService
  ) {}

  // @TODO: later add google invitation accept with state prop in v2 oauth
  @Get('google')
  @ApiTags('Auth')
  @CommonResponseDecorator(GetOauthDto)
  @Redirect('https://accounts.google.com/o/oauth2/v2/auth', 302)
  async googleAuth(@Query() query: { redirectUrl: string; invite?: string }): Promise<IOauthUrl> {
    const state = JSON.stringify({ redirectUrl: query.redirectUrl, invite: query.invite })

    const params = queryString.stringify({
      client_id: this.configService.get('GOOGLE_CLIENT_ID'),
      redirect_uri: `${this.configService.get('RUSHDB_DASHBOARD_URL')}/auth/google`,
      scope: [
        'https://www.googleapis.com/auth/userinfo.email',
        'https://www.googleapis.com/auth/userinfo.profile'
      ].join(' '),
      response_type: 'code',
      access_type: 'offline',
      prompt: 'consent',
      state
    })

    return { url: `https://accounts.google.com/o/oauth2/v2/auth?${params}` }
  }

  @Get('google/callback')
  @ApiTags('Auth')
  @CommonResponseDecorator(GetUserDto)
  @UseInterceptors(NeogmaTransactionInterceptor, NeogmaDataInterceptor, ChangeCorsInterceptor)
  async googleAuthRedirect(
    @TransactionDecorator() transaction: Transaction,
    @Query()
    params: { code: string; scope: string; authuser: string; prompt: string; state?: string }
  ) {
    try {
      let parsed: { redirectUrl?: string; invite?: string }

      try {
        parsed = JSON.parse(params.state ?? '{}')

        if (parsed.invite) {
          isDevMode(() => Logger.log(`[Google OAUTH LOG]: Has user invitation`))
        }
      } catch {
        throw new UnauthorizedException('Invalid OAuth state')
      }

      const user: User = await this.googleOAuthService.googleLogin(params.code, transaction)

      if (!user) {
        throw new UnauthorizedException()
      }

      const userData = user.toJson()

      if (!userData.confirmed) {
        await this.emailConfirmationService.sendVerificationLink(userData.login, userData.firstName)
      }

      return {
        ...userData,
        token: this.authService.createToken(user),
        inviteQuery: parsed.invite
      }
    } catch (e) {
      isDevMode(() => Logger.log(`[Google OAUTH ERROR]: `, e))

      throw new UnauthorizedException(e)
    }
  }
}
