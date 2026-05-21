import {
  BadRequestException,
  Body,
  Controller,
  Delete,
  Get,
  Param,
  Post,
  Query,
  Request,
  Res,
  UseInterceptors
} from '@nestjs/common'
import { ApiOperation, ApiTags } from '@nestjs/swagger'
import { FastifyReply } from 'fastify'

import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { PlatformRequest } from '@/common/types/request'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { AuthorizeAcceptDto } from '@/dashboard/mcp-oauth/dto/authorize-accept.dto'
import { RegisterClientDto } from '@/dashboard/mcp-oauth/dto/register-client.dto'
import { TokenRequestDto } from '@/dashboard/mcp-oauth/dto/token-request.dto'
import { McpOauthService } from '@/dashboard/mcp-oauth/mcp-oauth.service'
import { AuthUser } from '@/dashboard/user/decorators/user.decorator'
import { IUserClaims } from '@/dashboard/user/interfaces/user-claims.interface'

const TOKEN_EXCHANGE_GRANT = 'urn:ietf:params:oauth:grant-type:token-exchange'

@ApiTags('OAuth 2.1')
@Controller('oauth')
export class OAuthController {
  constructor(private readonly oauthService: McpOauthService) {}

  // ─────────────────────────────────────────────────────────────────────────────
  // Dynamic Client Registration (RFC 7591) — public
  // ─────────────────────────────────────────────────────────────────────────────

  @Post('register')
  @ApiOperation({ summary: 'Dynamic Client Registration (RFC 7591)' })
  async registerClient(@Body() dto: RegisterClientDto) {
    return this.oauthService.registerClient(dto)
  }

  // ─────────────────────────────────────────────────────────────────────────────
  // Authorization endpoint — public (redirects to dashboard consent UI)
  // ─────────────────────────────────────────────────────────────────────────────

  @Get('authorize')
  @ApiOperation({ summary: 'OAuth 2.1 Authorization endpoint — redirects to consent UI' })
  async authorize(
    @Query('response_type') response_type: string,
    @Query('client_id') client_id: string,
    @Query('redirect_uri') redirect_uri: string,
    @Query('scope') scope: string,
    @Query('code_challenge') code_challenge: string,
    @Query('code_challenge_method') code_challenge_method: string,
    @Query('state') state: string,
    @Query('resource') resource: string,
    @Res() reply: FastifyReply
  ) {
    const redirectUrl = await this.oauthService.startAuthorization({
      clientId: client_id,
      redirectUri: redirect_uri,
      scope,
      codeChallenge: code_challenge,
      codeChallengeMethod: code_challenge_method,
      state,
      resource
    })
    return reply.redirect(302, redirectUrl)
  }

  // ─────────────────────────────────────────────────────────────────────────────
  // Get auth request info (for consent UI display) — public
  // ─────────────────────────────────────────────────────────────────────────────

  @Get('authorize/request/:authRequestId')
  @UseInterceptors(TransformResponseInterceptor)
  @ApiOperation({ summary: 'Get authorization request metadata for consent UI' })
  async getAuthRequestInfo(@Param('authRequestId') authRequestId: string) {
    return this.oauthService.getAuthRequestInfo(authRequestId)
  }

  // ─────────────────────────────────────────────────────────────────────────────
  // Accept authorization (user submits consent) — requires dashboard JWT
  // ─────────────────────────────────────────────────────────────────────────────

  @Post('authorize/accept')
  @AuthGuard(null)
  @UseInterceptors(TransformResponseInterceptor)
  @ApiOperation({ summary: 'Submit user consent and receive redirect URL with authorization code' })
  async acceptAuthorization(@AuthUser() user: IUserClaims, @Body() dto: AuthorizeAcceptDto) {
    return this.oauthService.acceptAuthorization(user, dto)
  }

  // ─────────────────────────────────────────────────────────────────────────────
  // Deny authorization — requires dashboard JWT
  // ─────────────────────────────────────────────────────────────────────────────

  @Post('authorize/deny')
  @AuthGuard(null)
  @UseInterceptors(TransformResponseInterceptor)
  @ApiOperation({ summary: 'Deny an authorization request and redirect back with access_denied error' })
  async denyAuthorization(@Body() body: { authRequestId: string }) {
    return this.oauthService.denyAuthorization(body.authRequestId)
  }

  // ─────────────────────────────────────────────────────────────────────────────
  // Token endpoint — authorization_code + token exchange grants
  // ─────────────────────────────────────────────────────────────────────────────

  @Post('token')
  @ApiOperation({ summary: 'Token endpoint (RFC 6749 / RFC 8693)' })
  async token(@Body() dto: TokenRequestDto) {
    const { grant_type } = dto
    if (grant_type === 'authorization_code') {
      return this.oauthService.exchangeCode(dto)
    } else if (grant_type === TOKEN_EXCHANGE_GRANT) {
      return this.oauthService.exchangeToken(dto)
    } else if (grant_type === 'refresh_token') {
      return this.oauthService.handleRefreshToken(dto)
    } else {
      throw new BadRequestException(`Unsupported grant_type: ${grant_type}`)
    }
  }

  // ─────────────────────────────────────────────────────────────────────────────
  // Consent management — requires dashboard JWT
  // ─────────────────────────────────────────────────────────────────────────────

  @Get('consents')
  @AuthGuard(null)
  @UseInterceptors(TransformResponseInterceptor)
  @ApiOperation({ summary: 'List OAuth consents for the authenticated user' })
  async listConsents(@AuthUser() user: IUserClaims, @Request() request: PlatformRequest) {
    return this.oauthService.listConsents(user.id, request.workspaceId)
  }

  @Delete('consents/:consentId')
  @AuthGuard(null)
  @UseInterceptors(TransformResponseInterceptor)
  @ApiOperation({ summary: 'Revoke an OAuth consent' })
  async revokeConsent(@Param('consentId') consentId: string, @AuthUser() user: IUserClaims) {
    return this.oauthService.revokeConsent(consentId, user.id)
  }
}
