import { Controller, Get, Query, Redirect, UnauthorizedException, UseInterceptors } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { ApiTags } from '@nestjs/swagger'
import { Transaction } from 'neo4j-driver'
import * as queryString from 'query-string'

import { CommonResponseDecorator } from '@/common/decorators/common-response.decorator'
import { randomString } from '@/common/utils/randomString'
import { EntityService } from '@/core/entity/entity.service'
import { AuthService } from '@/dashboard/auth/auth.service'
import { IOauthUrl } from '@/dashboard/auth/auth.types'
import { GetOauthDto } from '@/dashboard/auth/dto/get-oauth.dto'
import { EmailConfirmationService } from '@/dashboard/auth/email-confirmation/email-confirmation.service'
import { GithubOAuthService } from '@/dashboard/auth/providers/github/github.service'
import { ChangeCorsInterceptor } from '@/dashboard/common/interceptors/change-cors.interceptor'
import { GetUserDto } from '@/dashboard/user/dto/get-user.dto'
import { UserService } from '@/dashboard/user/user.service'
import { NeogmaDataInterceptor } from '@/database/neogma/neogma-data.interceptor'
import { NeogmaTransactionInterceptor } from '@/database/neogma/neogma-transaction.interceptor'
import { TransactionDecorator } from '@/database/neogma/transaction.decorator'

@Controller('auth')
export class GithubOAuthController {
  constructor(
    private readonly userService: UserService,
    private readonly authService: AuthService,
    private readonly githubOAuthService: GithubOAuthService,
    private readonly configService: ConfigService,
    private readonly entityService: EntityService,
    private readonly emailConfirmationService: EmailConfirmationService
  ) {}

  @Get('github')
  @ApiTags('Auth')
  @CommonResponseDecorator(GetOauthDto)
  @Redirect('https://github.com/login/oauth/authorize', 302)
  async githubAuth(@Query() query: { redirectUrl: string }): Promise<IOauthUrl> {
    const params = queryString.stringify({
      client_id: this.configService.get('GH_CLIENT_ID'),
      // GITHUB ONLY ALLOWS FOR ONE CALLBACK URL
      redirect_uri: `${this.configService.get('GH_REDIRECT_URL')}?redirect_url=${query.redirectUrl}`,
      scope: 'user:email',
      state: randomString(32),
      allow_signup: 'true'
    })

    return { url: `https://github.com/login/oauth/authorize?${params}` }
  }

  @Get('github/callback')
  @ApiTags('Auth')
  @CommonResponseDecorator(GetUserDto)
  @Redirect()
  @UseInterceptors(NeogmaTransactionInterceptor, NeogmaDataInterceptor, ChangeCorsInterceptor)
  async githubAuthRedirect(
    @TransactionDecorator() transaction: Transaction,
    @Query()
    params: {
      code: string
      scope: string
      authuser: string
      prompt: string
      redirect_url: string
    }
  ) {
    const redirectTo = params.redirect_url ?? `${this.configService.get('RUSHDB_DASHBOARD_URL')}/auth/oauth`

    try {
      const user = await this.githubOAuthService.githubLogin(params.code, transaction)

      if (!user) {
        throw new UnauthorizedException()
      }

      const userData = user.toJson()

      if (!userData.confirmed) {
        await this.emailConfirmationService.sendVerificationLink(userData.login, userData.firstName)
      }

      const url = new URL(redirectTo)

      const token = this.authService.createToken(user)

      url.searchParams.append('token', token)

      return {
        url: url.toString()
      }
    } catch (e) {
      return new UnauthorizedException(e)
    }
  }
}
