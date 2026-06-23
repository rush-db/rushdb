import { BadRequestException, Inject, Injectable, Logger, NotFoundException } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { SAML } from '@node-saml/node-saml'
import { Transaction } from 'neo4j-driver'
import { uuidv7 } from 'uuidv7'

import { getCurrentISO } from '@/common/utils/getCurrentISO'
import { isDevMode } from '@/common/utils/isDevMode'
import { randomString } from '@/common/utils/randomString'
import { BILLING_POLICY_PORT, BillingPolicyPort } from '@/core/billing-policy/billing-policy.port'
import { AuditService } from '@/dashboard/auth/audit/audit.service'
import { AuthService } from '@/dashboard/auth/auth.service'
import { EncryptionService } from '@/dashboard/auth/encryption/encryption.service'
import { UpsertSsoConfigDto } from '@/dashboard/auth/providers/sso/dto/upsert-sso-config.dto'
import { SsoRepository } from '@/dashboard/auth/providers/sso/sso.repository'
import {
  TIdpConfig,
  TOidcState,
  TSamlRelayState,
  TSsoProfile
} from '@/dashboard/auth/providers/sso/sso.types'
import { USER_ROLE_EDITOR } from '@/dashboard/user/interfaces/user.constants'
import { TUserRoles } from '@/dashboard/user/model/user.interface'
import { UserService } from '@/dashboard/user/user.service'
import { WorkspaceService } from '@/dashboard/workspace/workspace.service'

import * as crypto from 'node:crypto'

import type { WorkspaceIdentityProviderRow } from '@/database/sql/schema/types'

const OIDC_SCOPE = 'openid email profile'

type OpenIdClient = typeof import('openid-client')

// `openid-client` v6 is pure ESM. The backend compiles to CommonJS and ships on
// Node 18 (see platform/Dockerfile), where a static `require()` of an ESM-only
// package throws ERR_REQUIRE_ESM. We load it through an indirect dynamic import
// built via `new Function` so TypeScript's CommonJS transform cannot downlevel
// it back into a `require()`. The module is cached after the first load.
let openIdClientPromise: Promise<OpenIdClient> | undefined
const loadOpenIdClient = (): Promise<OpenIdClient> => {
  if (!openIdClientPromise) {
    openIdClientPromise = new Function('return import("openid-client")')() as Promise<OpenIdClient>
  }
  return openIdClientPromise
}

@Injectable()
export class SsoService {
  private readonly logger = new Logger(SsoService.name)

  constructor(
    private readonly configService: ConfigService,
    private readonly ssoRepository: SsoRepository,
    private readonly userService: UserService,
    private readonly workspaceService: WorkspaceService,
    private readonly authService: AuthService,
    private readonly encryptionService: EncryptionService,
    private readonly auditService: AuditService,
    @Inject(BILLING_POLICY_PORT)
    private readonly billingPolicy: BillingPolicyPort
  ) {}

  // ── Config helpers ─────────────────────────────────────────────────────────

  private get baseUrl(): string {
    return this.configService.get('RUSHDB_BASE_URL') ?? 'http://localhost:3000'
  }

  get dashboardUrl(): string {
    return this.configService.get('RUSHDB_DASHBOARD_URL') ?? 'http://localhost:3005'
  }

  /** ACS / callback / metadata URLs handed to customer IT, keyed by config id. */
  samlAcsUrl(configId: string): string {
    return `${this.baseUrl}/api/v1/auth/sso/saml/${configId}/acs`
  }
  samlMetadataUrl(configId: string): string {
    return `${this.baseUrl}/api/v1/auth/sso/saml/${configId}/metadata`
  }
  samlSpEntityId(configId: string): string {
    return this.samlMetadataUrl(configId)
  }
  oidcCallbackUrl(configId: string): string {
    return `${this.baseUrl}/api/v1/auth/sso/oidc/${configId}/callback`
  }

  /** Backend login-initiation URL for a given provider (returned by /discover). */
  ssoLoginPath(config: TIdpConfig): string {
    return `${this.baseUrl}/api/v1/auth/sso/${config.type}/${config.id}/login`
  }

  /** Resolve a request path (with global prefix) to an absolute URL. */
  absoluteUrl(requestUrl: string): string {
    return `${this.baseUrl}${requestUrl}`
  }

  /** Dashboard page that reads the session token from the redirect and logs in. */
  dashboardCallbackUrl(token: string, workspaceId: string, redirectUrl?: string): string {
    const params = new URLSearchParams({ token, workspaceId })
    if (redirectUrl) {
      params.set('redirectUrl', redirectUrl)
    }
    return `${this.dashboardUrl}/auth/sso?${params.toString()}`
  }

  dashboardErrorUrl(type: string): string {
    return `${this.dashboardUrl}/signin?ssoError=${encodeURIComponent(type)}`
  }

  normalize(row: WorkspaceIdentityProviderRow): TIdpConfig {
    return {
      ...row,
      domains: this.safeJson<string[]>(row.domains, []),
      groupMappings: this.safeJson<Record<string, TUserRoles>>(row.groupMappings, null)
    }
  }

  private safeJson<T>(value: string | null, fallback: T): T {
    if (!value) {
      return fallback
    }
    try {
      return JSON.parse(value) as T
    } catch {
      return fallback
    }
  }

  // ── AES (state + OIDC secret) — mirrors workspace invite token encryption ───

  private aesEncrypt(plain: string): string {
    const key = this.configService.get('RUSHDB_AES_256_ENCRYPTION_KEY')
    const iv = crypto.randomBytes(16)
    const cipher = crypto.createCipheriv('aes-256-cbc', key, iv)
    return iv.toString('hex') + cipher.update(plain, 'utf8', 'base64') + cipher.final('base64')
  }

  private aesDecrypt(encrypted: string): string {
    const key = this.configService.get('RUSHDB_AES_256_ENCRYPTION_KEY')
    const normalized = decodeURIComponent(encrypted)
    const iv = normalized.substring(0, 32)
    const cipherText = normalized.substring(32)
    const decipher = crypto.createDecipheriv('aes-256-cbc', key, Buffer.from(iv, 'hex'))
    return decipher.update(cipherText, 'base64', 'utf8') + decipher.final('utf8')
  }

  // ── Admin: config CRUD ──────────────────────────────────────────────────────

  async listConfigs(workspaceId: string): Promise<ReturnType<SsoService['toPublic']>[]> {
    await this.billingPolicy.assertSsoAllowed(workspaceId)
    const rows = await this.ssoRepository.findByWorkspaceId(workspaceId)
    return rows.map((row) => this.toPublic(this.normalize(row)))
  }

  /** Public-safe projection — never exposes the encrypted OIDC secret. */
  toPublic(config: TIdpConfig) {
    return {
      id: config.id,
      workspaceId: config.workspaceId,
      type: config.type as 'saml' | 'oidc',
      enabled: config.enabled,
      enforced: config.enforced,
      domains: config.domains,
      defaultRole: config.defaultRole as TUserRoles,
      groupMappings: config.groupMappings,
      samlEntityId: config.samlEntityId,
      samlSsoUrl: config.samlSsoUrl,
      samlCertificate: config.samlCertificate,
      oidcIssuer: config.oidcIssuer,
      oidcClientId: config.oidcClientId,
      hasOidcClientSecret: Boolean(config.oidcClientSecretEncrypted),
      // SP values for customer IT
      sp: {
        acsUrl: this.samlAcsUrl(config.id),
        entityId: this.samlSpEntityId(config.id),
        metadataUrl: this.samlMetadataUrl(config.id),
        oidcRedirectUri: this.oidcCallbackUrl(config.id)
      },
      createdAt: config.createdAt,
      updatedAt: config.updatedAt
    }
  }

  async upsertConfig(workspaceId: string, dto: UpsertSsoConfigDto) {
    await this.billingPolicy.assertSsoAllowed(workspaceId)
    const now = getCurrentISO()
    const existing = await this.ssoRepository.findByWorkspaceAndType(workspaceId, dto.type)

    const domains = (dto.domains ?? []).map((d) => d.trim().toLowerCase()).filter(Boolean)

    // A domain may only be claimed by one workspace.
    await this.assertDomainsUnclaimed(domains, existing?.id)

    const oidcClientSecretEncrypted =
      dto.oidcClientSecret ? this.aesEncrypt(dto.oidcClientSecret) : existing?.oidcClientSecretEncrypted

    const data = {
      workspaceId,
      type: dto.type,
      enabled: dto.enabled ?? existing?.enabled ?? false,
      enforced: dto.enforced ?? existing?.enforced ?? false,
      domains: JSON.stringify(domains),
      defaultRole: dto.defaultRole ?? (existing?.defaultRole as TUserRoles) ?? USER_ROLE_EDITOR,
      groupMappings:
        dto.groupMappings ? JSON.stringify(dto.groupMappings) : (existing?.groupMappings ?? null),
      samlEntityId: dto.samlEntityId ?? existing?.samlEntityId ?? null,
      samlSsoUrl: dto.samlSsoUrl ?? existing?.samlSsoUrl ?? null,
      samlCertificate: dto.samlCertificate ?? existing?.samlCertificate ?? null,
      oidcIssuer: dto.oidcIssuer ?? existing?.oidcIssuer ?? null,
      oidcClientId: dto.oidcClientId ?? existing?.oidcClientId ?? null,
      oidcClientSecretEncrypted: oidcClientSecretEncrypted ?? null,
      updatedAt: now
    }

    const row =
      existing ?
        await this.ssoRepository.update(existing.id, data)
      : await this.ssoRepository.create({ id: uuidv7(), createdAt: now, ...data })

    this.auditService.record('sso.config.updated', { workspaceId, provider: dto.type })

    return this.toPublic(this.normalize(row))
  }

  async deleteConfig(workspaceId: string, configId: string): Promise<{ message: string }> {
    await this.billingPolicy.assertSsoAllowed(workspaceId)
    const row = await this.ssoRepository.findById(configId)
    if (!row || row.workspaceId !== workspaceId) {
      throw new NotFoundException('SSO configuration not found')
    }
    await this.ssoRepository.delete(configId)
    this.auditService.record('sso.config.deleted', { workspaceId, provider: row.type })
    return { message: 'SSO configuration removed' }
  }

  private async assertDomainsUnclaimed(domains: string[], ignoreConfigId?: string): Promise<void> {
    if (!domains.length) {
      return
    }
    const all = await this.ssoRepository.findAllEnabled()
    for (const row of all) {
      if (row.id === ignoreConfigId) {
        continue
      }
      const claimed = this.safeJson<string[]>(row.domains, [])
      const clash = domains.find((d) => claimed.includes(d))
      if (clash) {
        throw new BadRequestException(`Domain "${clash}" is already configured for SSO in another workspace`)
      }
    }
  }

  // ── Discovery ───────────────────────────────────────────────────────────────

  async findEnabledProviderForEmail(email: string): Promise<TIdpConfig | undefined> {
    const domain = email.split('@')[1]?.trim().toLowerCase()
    if (!domain) {
      return undefined
    }
    const all = await this.ssoRepository.findAllEnabled()
    const row = all.find((r) => this.safeJson<string[]>(r.domains, []).includes(domain))
    return row ? this.normalize(row) : undefined
  }

  async getConfigOrThrow(configId: string, type?: 'saml' | 'oidc'): Promise<TIdpConfig> {
    const row = await this.ssoRepository.findById(configId)
    if (!row || !row.enabled || (type && row.type !== type)) {
      throw new NotFoundException('SSO provider not found or disabled')
    }
    return this.normalize(row)
  }

  // ── SAML ─────────────────────────────────────────────────────────────────────

  private buildSaml(config: TIdpConfig): SAML {
    if (!config.samlSsoUrl || !config.samlCertificate) {
      throw new BadRequestException('SAML provider is not fully configured')
    }
    return new SAML({
      issuer: this.samlSpEntityId(config.id),
      callbackUrl: this.samlAcsUrl(config.id),
      entryPoint: config.samlSsoUrl,
      idpCert: config.samlCertificate,
      audience: this.samlSpEntityId(config.id),
      wantAssertionsSigned: true,
      wantAuthnResponseSigned: false,
      // We do not persist outgoing request ids across the stateless redirect.
      validateInResponseTo: 'never' as never,
      acceptedClockSkewMs: 5000
    })
  }

  async getSamlLoginUrl(config: TIdpConfig, redirectUrl?: string): Promise<string> {
    const saml = this.buildSaml(config)
    const relayState: TSamlRelayState = { configId: config.id, redirectUrl }
    return saml.getAuthorizeUrlAsync(this.aesEncrypt(JSON.stringify(relayState)), undefined, {})
  }

  async handleSamlAcs(
    config: TIdpConfig,
    body: { SAMLResponse?: string; RelayState?: string }
  ): Promise<{ profile: TSsoProfile; relayState: TSamlRelayState }> {
    if (!body?.SAMLResponse) {
      throw new BadRequestException('Missing SAMLResponse')
    }
    const saml = this.buildSaml(config)
    const { profile } = await saml.validatePostResponseAsync({
      SAMLResponse: body.SAMLResponse,
      RelayState: body.RelayState ?? ''
    })
    if (!profile) {
      throw new BadRequestException('Invalid SAML assertion')
    }

    let relayState: TSamlRelayState = { configId: config.id }
    if (body.RelayState) {
      try {
        relayState = JSON.parse(this.aesDecrypt(body.RelayState))
      } catch {
        /* keep default */
      }
    }

    return { profile: this.profileFromSaml(profile), relayState }
  }

  getSamlMetadata(config: TIdpConfig): string {
    const saml = this.buildSaml(config)
    return saml.generateServiceProviderMetadata(null, null)
  }

  private profileFromSaml(profile: Record<string, unknown>): TSsoProfile {
    const get = (...keys: string[]): string | undefined => {
      for (const k of keys) {
        const v = profile[k]
        if (typeof v === 'string' && v) {
          return v
        }
      }
      return undefined
    }
    const email =
      get(
        'email',
        'mail',
        'http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress',
        'urn:oid:0.9.2342.19200300.100.1.3'
      ) ?? (typeof profile.nameID === 'string' ? profile.nameID : undefined)

    if (!email) {
      throw new BadRequestException('SAML assertion did not include an email address')
    }

    return {
      subject: String(profile.nameID ?? email),
      email: email.toLowerCase(),
      firstName: get(
        'firstName',
        'givenName',
        'http://schemas.xmlsoap.org/ws/2005/05/identity/claims/givenname',
        'urn:oid:2.5.4.42'
      ),
      lastName: get(
        'lastName',
        'surname',
        'sn',
        'http://schemas.xmlsoap.org/ws/2005/05/identity/claims/surname',
        'urn:oid:2.5.4.4'
      ),
      groups: this.coerceGroups(
        profile['groups'] ??
          profile['Groups'] ??
          profile['http://schemas.xmlsoap.org/claims/Group'] ??
          profile['memberOf']
      )
    }
  }

  // ── OIDC ───────────────────────────────────────────────────────────────────

  private async oidcDiscovery(config: TIdpConfig): Promise<import('openid-client').Configuration> {
    if (!config.oidcIssuer || !config.oidcClientId) {
      throw new BadRequestException('OIDC provider is not fully configured')
    }
    const client = await loadOpenIdClient()
    const secret =
      config.oidcClientSecretEncrypted ? this.aesDecrypt(config.oidcClientSecretEncrypted) : undefined
    return client.discovery(new URL(config.oidcIssuer), config.oidcClientId, secret)
  }

  async getOidcLoginUrl(config: TIdpConfig, redirectUrl?: string): Promise<string> {
    const client = await loadOpenIdClient()
    const cfg = await this.oidcDiscovery(config)
    const codeVerifier = client.randomPKCECodeVerifier()
    const codeChallenge = await client.calculatePKCECodeChallenge(codeVerifier)
    const nonce = client.randomNonce()

    const state: TOidcState = { configId: config.id, codeVerifier, nonce, redirectUrl }
    const encodedState = this.aesEncrypt(JSON.stringify(state))

    const url = client.buildAuthorizationUrl(cfg, {
      redirect_uri: this.oidcCallbackUrl(config.id),
      scope: OIDC_SCOPE,
      code_challenge: codeChallenge,
      code_challenge_method: 'S256',
      nonce,
      state: encodedState
    })
    return url.href
  }

  async handleOidcCallback(
    config: TIdpConfig,
    currentUrl: string,
    rawState: string
  ): Promise<{ profile: TSsoProfile; redirectUrl?: string }> {
    let state: TOidcState
    try {
      state = JSON.parse(this.aesDecrypt(rawState))
    } catch {
      throw new BadRequestException('Invalid OIDC state')
    }

    const client = await loadOpenIdClient()
    const cfg = await this.oidcDiscovery(config)
    const tokens = await client.authorizationCodeGrant(cfg, new URL(currentUrl), {
      pkceCodeVerifier: state.codeVerifier,
      expectedNonce: state.nonce,
      expectedState: rawState
    })

    const claims = tokens.claims()
    if (!claims?.sub) {
      throw new BadRequestException('OIDC token did not include a subject')
    }

    let info: Record<string, unknown> = { ...claims }
    if (tokens.access_token) {
      try {
        info = { ...claims, ...(await client.fetchUserInfo(cfg, tokens.access_token, claims.sub)) }
      } catch {
        /* userinfo is best-effort; ID token claims are authoritative */
      }
    }

    const email = (info.email as string) ?? (claims.email as string)
    if (!email) {
      throw new BadRequestException('OIDC profile did not include an email address')
    }

    return {
      profile: {
        subject: String(claims.sub),
        email: email.toLowerCase(),
        firstName: (info.given_name as string) ?? undefined,
        lastName: (info.family_name as string) ?? undefined,
        groups: this.coerceGroups(info.groups ?? info.roles)
      },
      redirectUrl: state.redirectUrl
    }
  }

  private coerceGroups(value: unknown): string[] {
    if (Array.isArray(value)) {
      return value.map((v) => String(v))
    }
    if (typeof value === 'string') {
      return value
        .split(',')
        .map((s) => s.trim())
        .filter(Boolean)
    }
    return []
  }

  // ── Provisioning ──────────────────────────────────────────────────────────

  resolveRole(groups: string[], config: TIdpConfig): TUserRoles {
    if (config.groupMappings) {
      for (const group of groups) {
        const mapped = config.groupMappings[group]
        if (mapped) {
          return mapped
        }
      }
    }
    return (config.defaultRole as TUserRoles) ?? USER_ROLE_EDITOR
  }

  /**
   * Find-or-create the RushDB user from a verified SSO profile, ensure workspace
   * membership, and mint a session JWT. Email is trusted only because it comes
   * from a signature-verified assertion / ID token for a domain the workspace
   * has configured.
   */
  async provision(
    profile: TSsoProfile,
    config: TIdpConfig,
    transaction?: Transaction
  ): Promise<{ token: string; workspaceId: string }> {
    const role = this.resolveRole(profile.groups, config)
    const authField = config.type === 'saml' ? 'samlAuth' : 'oidcAuth'
    const subjectHash = await this.encryptionService.hash(profile.subject)

    let user = await this.userService.find(profile.email, transaction)

    if (!user) {
      user = await this.userService.createSsoUser(
        {
          login: profile.email,
          password: randomString(32),
          firstName: profile.firstName,
          lastName: profile.lastName,
          [authField]: subjectHash,
          confirmed: true
        },
        transaction
      )
      await this.workspaceService.attachUserToWorkspace(config.workspaceId, user.getId(), role, transaction)
      isDevMode(() =>
        Logger.log(`[SSO]: JIT-provisioned ${profile.email} into workspace ${config.workspaceId}`)
      )
    } else {
      await this.userService.update(user.getId(), { [authField]: subjectHash }, transaction)
      // Add to the workspace only if not already a member — never downgrade a
      // role that an admin set manually.
      await this.workspaceService.attachUserToWorkspaceIfAbsent(
        config.workspaceId,
        user.getId(),
        role,
        transaction
      )
    }

    this.auditService.record('sso.login.success', {
      workspaceId: config.workspaceId,
      userId: user.getId(),
      email: profile.email,
      provider: config.type
    })

    return { token: this.authService.createToken(user), workspaceId: config.workspaceId }
  }
}
