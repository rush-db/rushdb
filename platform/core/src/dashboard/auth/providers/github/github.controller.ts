import { Controller, Get, Query, Redirect, UnauthorizedException, UseInterceptors } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { ApiExcludeController, ApiTags } from '@nestjs/swagger'
import { Transaction } from 'neo4j-driver'
import * as queryString from 'query-string'

import { CommonResponseDecorator } from '@/common/decorators/common-response.decorator'
import { randomString } from '@/common/utils/randomString'
import { AuthService } from '@/dashboard/auth/auth.service'
import { IOauthUrl } from '@/dashboard/auth/auth.types'
import { GetOauthDto } from '@/dashboard/auth/dto/get-oauth.dto'
import { EmailConfirmationService } from '@/dashboard/auth/email-confirmation/email-confirmation.service'
import { GithubOAuthService } from '@/dashboard/auth/providers/github/github.service'
import { ChangeCorsInterceptor } from '@/dashboard/common/interceptors/change-cors.interceptor'
import { GetUserDto } from '@/dashboard/user/dto/get-user.dto'
import { DataInterceptor } from '@/database/interceptors/data.interceptor'
import { TransactionDecorator } from '@/database/transaction.decorator'

@Controller('auth')
@ApiExcludeController()
export class GithubOAuthController {
  constructor(
    private readonly authService: AuthService,
    private readonly githubOAuthService: GithubOAuthService,
    private readonly configService: ConfigService,
    private readonly emailConfirmationService: EmailConfirmationService
  ) {}

  @Get('github')
  @ApiTags('Auth')
  @CommonResponseDecorator(GetOauthDto)
  @Redirect('https://github.com/login/oauth/authorize', 302)
  async githubAuth(@Query() query: { redirectUrl: string }): Promise<IOauthUrl> {
    const params = queryString.stringify({
      client_id: this.configService.get('GH_CLIENT_ID'),
      redirect_uri: `${this.configService.get('RUSHDB_DASHBOARD_URL')}/auth/github`,
      scope: 'user:email',
      state: randomString(32),
      allow_signup: 'true'
    })

    return { url: `https://github.com/login/oauth/authorize?${params}` }
  }

  @Get('github/callback')
  @ApiTags('Auth')
  @CommonResponseDecorator(GetUserDto)
  @UseInterceptors(DataInterceptor, ChangeCorsInterceptor)
  async githubAuthRedirect(@TransactionDecorator() transaction: Transaction, @Query('code') code: string) {
    try {
      const user = await this.githubOAuthService.githubLogin(code, transaction)

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
      throw new UnauthorizedException()
    }
  }
}
