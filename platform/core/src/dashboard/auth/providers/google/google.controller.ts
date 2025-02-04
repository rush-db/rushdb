import { Controller, Get, Query, Redirect, UnauthorizedException, UseInterceptors } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { ApiTags } from '@nestjs/swagger'
import { Transaction } from 'neo4j-driver'
import * as queryString from 'query-string'

import { CommonResponseDecorator } from '@/common/decorators/common-response.decorator'
import { AuthService } from '@/dashboard/auth/auth.service'
import { IOauthUrl } from '@/dashboard/auth/auth.types'
import { GetOauthDto } from '@/dashboard/auth/dto/get-oauth.dto'
import { EmailConfirmationService } from '@/dashboard/auth/email-confirmation/email-confirmation.service'
import { GoogleOAuthService } from '@/dashboard/auth/providers/google/google.service'
import { ChangeCorsInterceptor } from '@/dashboard/common/interceptors/change-cors.interceptor'
import { GetUserDto } from '@/dashboard/user/dto/get-user.dto'
import { NeogmaDataInterceptor } from '@/database/neogma/neogma-data.interceptor'
import { NeogmaTransactionInterceptor } from '@/database/neogma/neogma-transaction.interceptor'
import { TransactionDecorator } from '@/database/neogma/transaction.decorator'

@Controller('auth')
export class GoogleOAuthController {
  constructor(
    private readonly authService: AuthService,
    private readonly googleOAuthService: GoogleOAuthService,
    private readonly configService: ConfigService,
    private readonly emailConfirmationService: EmailConfirmationService
  ) {}

  @Get('google')
  @ApiTags('Auth')
  @CommonResponseDecorator(GetOauthDto)
  @Redirect('https://accounts.google.com/o/oauth2/v2/auth', 302)
  async googleAuth(@Query() query: { redirectUrl: string }): Promise<IOauthUrl> {
    const params = queryString.stringify({
      client_id: this.configService.get('GOOGLE_CLIENT_ID'),
      redirect_uri: `${this.configService.get('RUSHDB_DASHBOARD_URL')}/auth/google`,
      scope: [
        'https://www.googleapis.com/auth/userinfo.email',
        'https://www.googleapis.com/auth/userinfo.profile'
      ].join(' '),
      response_type: 'code',
      access_type: 'offline',
      prompt: 'consent'
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
    params: { code: string; scope: string; authuser: string; prompt: string }
  ) {
    try {
      const user = await this.googleOAuthService.googleLogin(params.code, transaction)

      if (!user) {
        throw new UnauthorizedException()
      }

      const userData = user.toJson()

      if (!userData.confirmed) {
        await this.emailConfirmationService.sendVerificationLink(userData.login, userData.firstName)
      }

      return {
        ...userData,
        token: this.authService.createToken(user)
      }
    } catch (e) {
      return new UnauthorizedException(e)
    }
  }
}
