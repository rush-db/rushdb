import {
  BadRequestException,
  ForbiddenException,
  Injectable,
  Logger,
  NotFoundException,
  UnauthorizedException
} from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { JwtService } from '@nestjs/jwt'
import { uuidv7 } from 'uuidv7'

import { AuthService } from '@/dashboard/auth/auth.service'
import { AuthorizeAcceptDto } from '@/dashboard/mcp-oauth/dto/authorize-accept.dto'
import { RegisterClientDto } from '@/dashboard/mcp-oauth/dto/register-client.dto'
import { TokenRequestDto } from '@/dashboard/mcp-oauth/dto/token-request.dto'
import { OAuthRepository } from '@/dashboard/mcp-oauth/model/oauth.repository'
import { ProjectService } from '@/dashboard/project/project.service'
import { TokenService } from '@/dashboard/token/token.service'
import { IUserClaims } from '@/dashboard/user/interfaces/user-claims.interface'

import { createHash } from 'node:crypto'

// Label constants kept for potential external references
export const LABEL_OAUTH_CLIENT = '__RUSHDB__LABEL__OAUTH_CLIENT__'
export const LABEL_OAUTH_AUTH_REQUEST = '__RUSHDB__LABEL__OAUTH_AUTH_REQUEST__'
export const LABEL_OAUTH_CONSENT = '__RUSHDB__LABEL__OAUTH_CONSENT__'
export const LABEL_OAUTH_CODE = '__RUSHDB__LABEL__OAUTH_CODE__'

const AUTH_REQUEST_TTL_MS = 10 * 60 * 1000 // 10 minutes
const AUTH_CODE_TTL_MS = 5 * 60 * 1000 // 5 minutes
const ACCESS_TOKEN_TTL_S = 3600 // 1 hour
const PROJECT_TOKEN_TTL = '1h'

const SUPPORTED_SCOPES = ['records:read', 'records:write']
const TOKEN_EXCHANGE_GRANT = 'urn:ietf:params:oauth:grant-type:token-exchange'
const ACCESS_TOKEN_TYPE = 'urn:ietf:params:oauth:token-type:access_token'

@Injectable()
export class McpOauthService {
  private readonly logger = new Logger(McpOauthService.name)

  private readonly issuer: string

  constructor(
    private readonly jwtService: JwtService,
    private readonly authService: AuthService,
    private readonly tokenService: TokenService,
    private readonly projectService: ProjectService,
    private readonly configService: ConfigService,
    private readonly oauthRepository: OAuthRepository
  ) {
    this.issuer = this.configService.get<string>('RUSHDB_PUBLIC_URL') || 'https://api.rushdb.com'
  }

  // ---------------------------------------------------------------------------
  // RFC 8414 - Discovery document
  // ---------------------------------------------------------------------------

  getDiscoveryDocument() {
    const base = this.issuer.replace(/\/$/, '')
    return {
      issuer: base,
      authorization_endpoint: `${base}/oauth/authorize`,
      token_endpoint: `${base}/oauth/token`,
      registration_endpoint: `${base}/oauth/register`,
      scopes_supported: SUPPORTED_SCOPES,
      response_types_supported: ['code'],
      grant_types_supported: ['authorization_code', TOKEN_EXCHANGE_GRANT],
      code_challenge_methods_supported: ['S256'],
      token_endpoint_auth_methods_supported: ['none'],
      revocation_endpoint: `${base}/oauth/revoke`
    }
  }

  // ---------------------------------------------------------------------------
  // Dynamic client registration (RFC 7591)
  // ---------------------------------------------------------------------------

  async registerClient(dto: RegisterClientDto) {
    const {
      client_name: clientName,
      redirect_uris: redirectUris,
      token_endpoint_auth_method: tokenEndpointAuthMethod = 'none',
      application_type: applicationType = 'web'
    } = dto

    if (!clientName || !redirectUris?.length) {
      throw new BadRequestException('client_name and redirect_uris are required')
    }

    const sortedRedirectUris = [...redirectUris].sort().join(',')

    // upsertClient returns the client id (string)
    const clientId = await this.oauthRepository.upsertClient({
      clientName,
      redirectUris: sortedRedirectUris,
      tokenEndpointAuthMethod,
      applicationType
    })

    return {
      client_id: clientId,
      client_name: clientName,
      redirect_uris: redirectUris,
      token_endpoint_auth_method: tokenEndpointAuthMethod,
      application_type: applicationType
    }
  }

  // ---------------------------------------------------------------------------
  // Authorization endpoint helpers
  // ---------------------------------------------------------------------------

  async startAuthorization(dto: {
    clientId: string
    redirectUri: string
    scope: string
    resource?: string
    codeChallenge?: string
    codeChallengeMethod?: string
    state?: string
  }): Promise<string> {
    const { clientId, redirectUri, scope, resource, codeChallenge, codeChallengeMethod, state } = dto

    const client = await this.getClient(clientId)
    if (!client) {
      throw new NotFoundException(`Unknown client: ${clientId}`)
    }

    const allowedUris = client.redirectUris.split(',')
    if (!allowedUris.includes(redirectUri)) {
      throw new ForbiddenException(`Redirect URI not registered for client ${clientId}`)
    }

    const requestedScopes = (scope || '').split(' ').filter(Boolean)
    const validScopes = requestedScopes.filter((s) => SUPPORTED_SCOPES.includes(s))
    if (!validScopes.length) {
      throw new BadRequestException(`No valid scopes requested. Supported: ${SUPPORTED_SCOPES.join(', ')}`)
    }

    const now = Date.now()
    const id = 'oauthr_' + uuidv7()
    const expiresAt = new Date(now + AUTH_REQUEST_TTL_MS).toISOString()

    await this.oauthRepository.createAuthRequest({
      id,
      clientId,
      redirectUri,
      scope: validScopes.join(' '),
      resource: resource || null,
      // codeChallenge is notNull in schema - use empty string when PKCE not provided
      codeChallenge: codeChallenge ?? '',
      codeChallengeMethod: codeChallengeMethod || 'plain',
      state: state || null,
      created: new Date(now).toISOString(),
      expiresAt
    })

    // Redirect to dashboard consent UI
    const dashboardUrl = this.configService.get<string>('RUSHDB_DASHBOARD_URL') || 'http://localhost:3005'
    const consentUrl = new URL(`${dashboardUrl}/oauth/consent`)
    consentUrl.searchParams.set('request_id', id)
    return consentUrl.toString()
  }

  async getAuthRequestInfo(authRequestId: string) {
    const authRequest = await this.getAuthRequest(authRequestId)
    if (!authRequest) {
      throw new NotFoundException('Authorization request not found or expired')
    }

    const client = await this.getClient(authRequest.clientId)
    if (!client) {
      throw new NotFoundException('Client not found')
    }

    return {
      auth_request_id: authRequest.id,
      client_id: authRequest.clientId,
      client_name: client.clientName,
      redirect_uri: authRequest.redirectUri,
      scope: authRequest.scope,
      resource: authRequest.resource,
      state: authRequest.state
    }
  }

  // Note: controller calls acceptAuthorization(user, dto) - user is first param
  async acceptAuthorization(user: IUserClaims, dto: AuthorizeAcceptDto) {
    const authRequestId = dto.auth_request_id
    const projectId = dto.project_id
    const userId = user.id

    const authRequest = await this.getAuthRequest(authRequestId)
    if (!authRequest) {
      throw new NotFoundException('Authorization request not found or expired')
    }

    // Verify the user actually has access to the project
    const hasAccess = await this.authService.hasProjectAccess(projectId, userId)
    if (!hasAccess) {
      throw new ForbiddenException(`User does not have access to project ${projectId}`)
    }

    // Resolve the granted scopes: use user-selected subset if provided, else all requested scopes
    const requestedScopes = (authRequest.scope || '').split(' ').filter(Boolean)
    const grantedScopes =
      dto.scope ?
        dto.scope
          .split(' ')
          .filter(Boolean)
          .filter((s) => requestedScopes.includes(s))
      : requestedScopes
    if (grantedScopes.length === 0) {
      throw new BadRequestException('At least one permission must be selected')
    }
    const grantedScope = grantedScopes.join(' ')

    // Retrieve or create a consent record
    let consent = await this.oauthRepository.findActiveConsent(userId, authRequest.clientId, projectId)

    if (!consent) {
      const consentId = 'oauthconsent_' + uuidv7()
      await this.oauthRepository.createConsent({
        id: consentId,
        userId,
        clientId: authRequest.clientId,
        projectId,
        scope: grantedScope,
        resource: authRequest.resource || null,
        created: new Date().toISOString(),
        revokedAt: null
      })
      consent = await this.oauthRepository.findConsentById(consentId)
    }

    // Issue authorization code
    const codeId = 'oauthcode_' + uuidv7()
    await this.oauthRepository.createCode({
      id: codeId,
      consentId: consent.id,
      clientId: authRequest.clientId,
      redirectUri: authRequest.redirectUri,
      codeChallenge: authRequest.codeChallenge ?? '',
      codeChallengeMethod: authRequest.codeChallengeMethod || 'plain',
      resource: authRequest.resource || null,
      scope: grantedScope || null,
      created: new Date().toISOString(),
      expiresAt: new Date(Date.now() + AUTH_CODE_TTL_MS).toISOString()
    })

    // Clean up the auth request
    await this.oauthRepository.deleteAuthRequest(authRequestId)

    const redirectUrl = new URL(authRequest.redirectUri)
    redirectUrl.searchParams.set('code', codeId)
    if (authRequest.state) {
      redirectUrl.searchParams.set('state', authRequest.state)
    }

    return { redirectTo: redirectUrl.toString() }
  }

  async denyAuthorization(authRequestId: string) {
    const authRequest = await this.getAuthRequest(authRequestId)
    if (!authRequest) {
      throw new NotFoundException('Authorization request not found')
    }

    await this.oauthRepository.deleteAuthRequest(authRequestId)

    const redirectUrl = new URL(authRequest.redirectUri)
    redirectUrl.searchParams.set('error', 'access_denied')
    if (authRequest.state) {
      redirectUrl.searchParams.set('state', authRequest.state)
    }

    return { redirectTo: redirectUrl.toString() }
  }

  // ---------------------------------------------------------------------------
  // Token endpoint - authorization_code grant
  // ---------------------------------------------------------------------------

  async exchangeCode(dto: TokenRequestDto) {
    const { code, code_verifier } = dto

    if (!code) {
      throw new BadRequestException('code is required')
    }

    const codeRow = await this.oauthRepository.findCode(code)
    if (!codeRow) {
      throw new BadRequestException('Invalid or expired authorization code')
    }

    // PKCE verification (skip if codeChallenge is empty - PKCE was optional)
    if (codeRow.codeChallenge) {
      if (!code_verifier) {
        throw new BadRequestException('code_verifier is required')
      }

      const method = codeRow.codeChallengeMethod || 'plain'
      let derived: string

      if (method === 'S256') {
        derived = createHash('sha256').update(code_verifier).digest('base64url')
      } else {
        derived = code_verifier
      }

      if (derived !== codeRow.codeChallenge) {
        throw new ForbiddenException('PKCE verification failed')
      }
    }

    const consentRow = await this.oauthRepository.findConsentById(codeRow.consentId)
    if (!consentRow || consentRow.revokedAt) {
      throw new UnauthorizedException('Consent has been revoked or does not exist')
    }

    // Code is single-use - delete immediately
    await this.oauthRepository.deleteCode(code)

    // Issue JWT access token
    const accessToken = this.jwtService.sign(
      {
        sub: consentRow.userId,
        scope: consentRow.scope,
        project_id: consentRow.projectId,
        consent_id: codeRow.consentId,
        aud: codeRow.resource || this.issuer
      },
      { expiresIn: ACCESS_TOKEN_TTL_S }
    )

    return {
      access_token: accessToken,
      token_type: 'bearer',
      expires_in: ACCESS_TOKEN_TTL_S,
      scope: consentRow.scope
    }
  }

  // ---------------------------------------------------------------------------
  // Token endpoint - token exchange grant (RFC 8693)
  // ---------------------------------------------------------------------------

  async exchangeToken(dto: TokenRequestDto) {
    const { subject_token, project_id } = dto

    if (!subject_token) {
      throw new BadRequestException('subject_token is required')
    }
    if (!project_id) {
      throw new BadRequestException('project_id is required for token exchange')
    }

    // Verify the OAuth JWT
    let payload: any
    try {
      payload = this.jwtService.verify(subject_token)
    } catch (e) {
      throw new UnauthorizedException('Invalid or expired subject_token')
    }

    if (payload.project_id !== project_id) {
      throw new ForbiddenException(`project_id ${project_id} is not authorized for this token`)
    }

    const scopes: string[] = (payload.scope || '').split(' ').filter(Boolean)
    const level: 'read' | 'write' = scopes.includes('records:write') ? 'write' : 'read'

    // Dedup: reuse an existing live token for this consent+project
    if (payload.consent_id) {
      const existingToken = await this.tokenService.findLiveTokenByConsentAndProject(
        payload.consent_id,
        project_id
      )
      if (existingToken) {
        this.logger.debug(
          `[OAuth] reusing existing token for consent ${payload.consent_id} / project ${project_id}`
        )
        return {
          access_token: existingToken.value,
          token_type: 'bearer',
          expires_in: ACCESS_TOKEN_TTL_S,
          scope: payload.scope,
          project_id,
          issued_token_type: ACCESS_TOKEN_TYPE
        }
      }
    }

    const tokenEntity = await this.tokenService.createToken(
      {
        name: `OAuth exchange [${payload.sub}] -> ${project_id}`,
        description: `Issued via OAuth token exchange. Consent: ${payload.consent_id || 'unknown'}`,
        expiration: PROJECT_TOKEN_TTL,
        level,
        issuedBy: 'oauth_exchange',
        consentId: payload.consent_id,
        scopes: payload.scope
      },
      project_id
    )

    return {
      access_token: tokenEntity.toJson().value,
      token_type: 'bearer',
      expires_in: ACCESS_TOKEN_TTL_S,
      scope: payload.scope,
      project_id,
      issued_token_type: ACCESS_TOKEN_TYPE
    }
  }

  // ---------------------------------------------------------------------------
  // Consent management
  // ---------------------------------------------------------------------------

  async listConsents(userId: string, workspaceId?: string) {
    const consents = await this.oauthRepository.listActiveConsentsWithDetails(userId, workspaceId)
    return consents.map((c) => ({
      id: c.id,
      client_id: c.clientId,
      client_name: c.clientName ?? c.clientId,
      scope: c.scope,
      project_id: c.projectId,
      project_name: c.projectName ?? c.projectId,
      resource: c.resource,
      created: c.created
    }))
  }

  async revokeConsent(consentId: string, userId: string) {
    const consent = await this.oauthRepository.findConsentById(consentId)
    if (!consent || consent.userId !== userId) {
      throw new NotFoundException('Consent not found')
    }

    await this.oauthRepository.revokeConsent(consentId)
    await this.tokenService.deleteByConsentId(consentId)

    return { success: true }
  }

  // ---------------------------------------------------------------------------
  // Helpers
  // ---------------------------------------------------------------------------

  private async getClient(clientId: string) {
    return this.oauthRepository.findClientById(clientId)
  }

  private async getAuthRequest(authRequestId: string) {
    return this.oauthRepository.findAuthRequest(authRequestId)
  }
}
