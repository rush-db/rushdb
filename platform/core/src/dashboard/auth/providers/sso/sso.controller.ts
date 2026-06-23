import { Body, Controller, Get, Logger, Param, Post, Query, Req, Res } from '@nestjs/common'
import { ApiExcludeController, ApiTags } from '@nestjs/swagger'
import { FastifyReply, FastifyRequest } from 'fastify'

import { isDevMode } from '@/common/utils/isDevMode'
import { AuditService } from '@/dashboard/auth/audit/audit.service'
import { SsoDiscoverDto } from '@/dashboard/auth/providers/sso/dto/sso-discover.dto'
import { SsoService } from '@/dashboard/auth/providers/sso/sso.service'
import { TIdpConfig } from '@/dashboard/auth/providers/sso/sso.types'

@Controller('auth/sso')
@ApiExcludeController()
export class SsoController {
  constructor(
    private readonly ssoService: SsoService,
    private readonly auditService: AuditService
  ) {}

  /** Step 1: dashboard posts the email; we resolve the IdP by domain. */
  @Post('discover')
  @ApiTags('Auth')
  async discover(@Body() { email }: SsoDiscoverDto, @Query('redirectUrl') redirectUrl?: string) {
    const config = await this.ssoService.findEnabledProviderForEmail(email)
    if (!config) {
      return { ssoAvailable: false }
    }
    const params = redirectUrl ? `?redirectUrl=${encodeURIComponent(redirectUrl)}` : ''
    return {
      ssoAvailable: true,
      type: config.type,
      enforced: config.enforced,
      loginUrl: `${this.ssoService.ssoLoginPath(config)}${params}`
    }
  }

  // ── SAML ───────────────────────────────────────────────────────────────────

  @Get('saml/:configId/login')
  @ApiTags('Auth')
  async samlLogin(
    @Param('configId') configId: string,
    @Res() reply: FastifyReply,
    @Query('redirectUrl') redirectUrl?: string
  ) {
    const config = await this.ssoService.getConfigOrThrow(configId, 'saml')
    const url = await this.ssoService.getSamlLoginUrl(config, redirectUrl)
    return reply.redirect(url, 302)
  }

  @Post('saml/:configId/acs')
  @ApiTags('Auth')
  async samlAcs(
    @Param('configId') configId: string,
    @Body() body: { SAMLResponse?: string; RelayState?: string },
    @Res() reply: FastifyReply
  ) {
    const config = await this.ssoService.getConfigOrThrow(configId, 'saml')
    try {
      const { profile, relayState } = await this.ssoService.handleSamlAcs(config, body)
      const { token, workspaceId } = await this.ssoService.provision(profile, config)
      return reply.redirect(
        this.ssoService.dashboardCallbackUrl(token, workspaceId, relayState.redirectUrl),
        302
      )
    } catch (e) {
      isDevMode(() => Logger.error('[SSO SAML ACS ERROR]', e as Error))
      this.auditService.record('sso.login.failed', {
        workspaceId: config.workspaceId,
        provider: 'saml',
        reason: (e as Error)?.message
      })
      return reply.redirect(this.ssoService.dashboardErrorUrl('saml'), 302)
    }
  }

  @Get('saml/:configId/metadata')
  @ApiTags('Auth')
  async samlMetadata(@Param('configId') configId: string, @Res() reply: FastifyReply) {
    const config = await this.ssoService.getConfigOrThrow(configId, 'saml')
    const xml = this.ssoService.getSamlMetadata(config)
    return reply.type('application/xml').send(xml)
  }

  // ── OIDC ───────────────────────────────────────────────────────────────────

  @Get('oidc/:configId/login')
  @ApiTags('Auth')
  async oidcLogin(
    @Param('configId') configId: string,
    @Res() reply: FastifyReply,
    @Query('redirectUrl') redirectUrl?: string
  ) {
    const config = await this.ssoService.getConfigOrThrow(configId, 'oidc')
    const url = await this.ssoService.getOidcLoginUrl(config, redirectUrl)
    return reply.redirect(url, 302)
  }

  @Get('oidc/:configId/callback')
  @ApiTags('Auth')
  async oidcCallback(
    @Param('configId') configId: string,
    @Query('state') state: string,
    @Req() request: FastifyRequest,
    @Res() reply: FastifyReply
  ) {
    let config: TIdpConfig
    try {
      config = await this.ssoService.getConfigOrThrow(configId, 'oidc')
      const currentUrl = `${this.ssoService.absoluteUrl(request.url)}`
      const { profile, redirectUrl } = await this.ssoService.handleOidcCallback(config, currentUrl, state)
      const { token, workspaceId } = await this.ssoService.provision(profile, config)
      return reply.redirect(this.ssoService.dashboardCallbackUrl(token, workspaceId, redirectUrl), 302)
    } catch (e) {
      isDevMode(() => Logger.error('[SSO OIDC CALLBACK ERROR]', e as Error))
      this.auditService.record('sso.login.failed', {
        workspaceId: config?.workspaceId,
        provider: 'oidc',
        reason: (e as Error)?.message
      })
      return reply.redirect(this.ssoService.dashboardErrorUrl('oidc'), 302)
    }
  }
}
