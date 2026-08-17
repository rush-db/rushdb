import {
  Controller,
  Get,
  Logger,
  Query,
  Redirect,
  Req,
  Res,
  UnauthorizedException,
  UseInterceptors
} from '@nestjs/common'
import { ApiExcludeController, ApiTags } from '@nestjs/swagger'
import { FastifyReply, FastifyRequest } from 'fastify'
import { Transaction } from 'neo4j-driver'

import { CommonResponseDecorator } from '@/common/decorators/common-response.decorator'
import { isDevMode } from '@/common/utils/isDevMode'
import { AuthService } from '@/dashboard/auth/auth.service'
import { IOauthUrl } from '@/dashboard/auth/auth.types'
import { GetOauthDto } from '@/dashboard/auth/dto/get-oauth.dto'
import { GithubOAuthService } from '@/dashboard/auth/providers/github/github.service'
import { ChangeCorsInterceptor } from '@/dashboard/common/interceptors/change-cors.interceptor'
import { GetUserDto } from '@/dashboard/user/dto/get-user.dto'
import { DataInterceptor } from '@/database/interceptors/data.interceptor'
import { TransactionDecorator } from '@/database/transaction.decorator'

import { createHash, randomBytes, timingSafeEqual } from 'node:crypto'

const GITHUB_OAUTH_COOKIE = 'rushdb_github_oauth'
const GITHUB_OAUTH_COOKIE_PATH = '/api/v1/auth/github'
const GITHUB_OAUTH_MAX_AGE_SECONDS = 10 * 60

type GithubOauthCookie = {
  state: string
  codeVerifier: string
}

const serializeOauthCookie = (value: string, secure: boolean, maxAge: number): string =>
  [
    `${GITHUB_OAUTH_COOKIE}=${encodeURIComponent(value)}`,
    `Path=${GITHUB_OAUTH_COOKIE_PATH}`,
    'HttpOnly',
    'SameSite=Lax',
    `Max-Age=${maxAge}`,
    ...(secure ? ['Secure'] : [])
  ].join('; ')

const readOauthCookie = (request: FastifyRequest): GithubOauthCookie | undefined => {
  const value = request.headers.cookie
    ?.split(';')
    .map((part) => part.trim())
    .find((part) => part.startsWith(`${GITHUB_OAUTH_COOKIE}=`))
    ?.slice(GITHUB_OAUTH_COOKIE.length + 1)

  if (!value) {
    return undefined
  }

  try {
    return JSON.parse(Buffer.from(decodeURIComponent(value), 'base64url').toString('utf8'))
  } catch {
    return undefined
  }
}

const statesMatch = (expected?: string, actual?: string): boolean => {
  if (!expected || !actual) {
    return false
  }

  const expectedBuffer = Buffer.from(expected)
  const actualBuffer = Buffer.from(actual)
  return expectedBuffer.length === actualBuffer.length && timingSafeEqual(expectedBuffer, actualBuffer)
}

@Controller('auth')
@ApiExcludeController()
export class GithubOAuthController {
  constructor(
    private readonly authService: AuthService,
    private readonly githubOAuthService: GithubOAuthService
  ) {}

  @Get('github')
  @ApiTags('Auth')
  @CommonResponseDecorator(GetOauthDto)
  @Redirect('https://github.com/login/oauth/authorize', 302)
  async githubAuth(@Res({ passthrough: true }) reply: FastifyReply): Promise<IOauthUrl> {
    const state = randomBytes(32).toString('base64url')
    const codeVerifier = randomBytes(48).toString('base64url')
    const redirectUri = this.githubOAuthService.getRedirectUri()
    const cookie = Buffer.from(JSON.stringify({ state, codeVerifier })).toString('base64url')

    reply.header(
      'Set-Cookie',
      serializeOauthCookie(cookie, redirectUri.startsWith('https://'), GITHUB_OAUTH_MAX_AGE_SECONDS)
    )

    const params = new URLSearchParams({
      client_id: this.githubOAuthService.getClientId(),
      redirect_uri: redirectUri,
      scope: 'user:email',
      state,
      allow_signup: 'true',
      code_challenge: createHash('sha256').update(codeVerifier).digest('base64url'),
      code_challenge_method: 'S256'
    })

    return { url: `https://github.com/login/oauth/authorize?${params.toString()}` }
  }

  @Get('github/callback')
  @ApiTags('Auth')
  @CommonResponseDecorator(GetUserDto)
  @UseInterceptors(DataInterceptor, ChangeCorsInterceptor)
  async githubAuthRedirect(
    @TransactionDecorator() transaction: Transaction,
    @Query() params: { code?: string; state?: string; error?: string },
    @Req() request: FastifyRequest,
    @Res({ passthrough: true }) reply: FastifyReply
  ) {
    const redirectUri = this.githubOAuthService.getRedirectUri()
    const oauthCookie = readOauthCookie(request)

    // Make state and PKCE verifier single-use even when validation or exchange fails.
    reply.header('Set-Cookie', serializeOauthCookie('', redirectUri.startsWith('https://'), 0))

    try {
      if (params.error) {
        throw new UnauthorizedException('GitHub authorization was denied')
      }

      if (!params.code || !statesMatch(oauthCookie?.state, params.state) || !oauthCookie?.codeVerifier) {
        throw new UnauthorizedException('Invalid or expired GitHub OAuth state')
      }

      const user = await this.githubOAuthService.githubLogin(
        params.code,
        oauthCookie.codeVerifier,
        transaction
      )
      const userData = user.toJson()

      return {
        ...userData,
        token: this.authService.createToken(user)
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Unknown GitHub OAuth error'
      isDevMode(() => Logger.warn(`[GitHub OAuth] ${message}`))

      if (error instanceof UnauthorizedException) {
        throw error
      }

      throw new UnauthorizedException('GitHub authentication failed')
    }
  }
}
