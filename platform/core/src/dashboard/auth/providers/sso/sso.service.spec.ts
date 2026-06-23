import { SsoService } from './sso.service'

import type { TIdpConfig } from './sso.types'
import type { WorkspaceIdentityProviderRow } from '@/database/sql/schema/types'

const AES_KEY = '32SymbolStringForTokenEncryption' // exactly 32 chars

const makeRow = (over: Partial<WorkspaceIdentityProviderRow> = {}): WorkspaceIdentityProviderRow =>
  ({
    id: 'cfg-1',
    workspaceId: 'ws-1',
    type: 'saml',
    enabled: true,
    enforced: false,
    domains: JSON.stringify(['acme.com']),
    defaultRole: 'developer',
    groupMappings: null,
    samlEntityId: null,
    samlSsoUrl: 'https://idp.acme.com/sso',
    samlCertificate: 'CERT',
    oidcIssuer: null,
    oidcClientId: null,
    oidcClientSecretEncrypted: null,
    createdAt: '2026-01-01T00:00:00.000Z',
    updatedAt: '2026-01-01T00:00:00.000Z',
    ...over
  }) as WorkspaceIdentityProviderRow

const makeConfig = (over: Partial<TIdpConfig> = {}): TIdpConfig => ({
  ...(makeRow() as unknown as TIdpConfig),
  domains: ['acme.com'],
  groupMappings: null,
  ...over
})

describe('SsoService', () => {
  let ssoRepository: {
    findAllEnabled: jest.Mock
    findByWorkspaceId: jest.Mock
    findByWorkspaceAndType: jest.Mock
    findById: jest.Mock
    delete: jest.Mock
  }
  let billingPolicy: { assertSsoAllowed: jest.Mock }
  let service: SsoService

  beforeEach(() => {
    ssoRepository = {
      findAllEnabled: jest.fn(),
      findByWorkspaceId: jest.fn().mockResolvedValue([]),
      findByWorkspaceAndType: jest.fn().mockResolvedValue(undefined),
      findById: jest.fn(),
      delete: jest.fn()
    }
    billingPolicy = { assertSsoAllowed: jest.fn().mockResolvedValue(undefined) }
    const configService = {
      get: (key: string) => {
        if (key === 'RUSHDB_AES_256_ENCRYPTION_KEY') {
          return AES_KEY
        }
        if (key === 'RUSHDB_BASE_URL') {
          return 'https://api.rushdb.com'
        }
        if (key === 'RUSHDB_DASHBOARD_URL') {
          return 'https://app.rushdb.com'
        }
        return undefined
      }
    }
    service = new SsoService(
      configService as never,
      ssoRepository as never,
      {} as never,
      {} as never,
      {} as never,
      {} as never,
      {} as never,
      billingPolicy as never
    )
  })

  describe('plan gating', () => {
    it('lets eligible workspaces list configs', async () => {
      await expect(service.listConfigs('ws-1')).resolves.toEqual([])
      expect(billingPolicy.assertSsoAllowed).toHaveBeenCalledWith('ws-1')
    })

    it('blocks listing when the plan is not allowed', async () => {
      billingPolicy.assertSsoAllowed.mockRejectedValueOnce(
        new Error('SSO requires the Scale plan or higher.')
      )
      await expect(service.listConfigs('ws-1')).rejects.toThrow('Scale plan')
      expect(ssoRepository.findByWorkspaceId).not.toHaveBeenCalled()
    })

    it('gates upsertConfig before touching the repository', async () => {
      billingPolicy.assertSsoAllowed.mockRejectedValueOnce(
        new Error('SSO requires the Scale plan or higher.')
      )
      await expect(service.upsertConfig('ws-1', { type: 'saml' } as never)).rejects.toThrow('Scale plan')
      expect(ssoRepository.findByWorkspaceAndType).not.toHaveBeenCalled()
    })

    it('gates deleteConfig before touching the repository', async () => {
      billingPolicy.assertSsoAllowed.mockRejectedValueOnce(
        new Error('SSO requires the Scale plan or higher.')
      )
      await expect(service.deleteConfig('ws-1', 'cfg-1')).rejects.toThrow('Scale plan')
      expect(ssoRepository.findById).not.toHaveBeenCalled()
    })
  })

  describe('resolveRole', () => {
    it('falls back to the default role with no group mappings', () => {
      const config = makeConfig({ defaultRole: 'viewer' })
      expect(service.resolveRole(['anything'], config)).toBe('viewer')
    })

    it('maps an IdP group to a role when mapped', () => {
      const config = makeConfig({
        defaultRole: 'viewer',
        groupMappings: { 'rushdb-admins': 'admin', 'rushdb-devs': 'developer' }
      })
      expect(service.resolveRole(['rushdb-admins'], config)).toBe('admin')
      expect(service.resolveRole(['rushdb-devs'], config)).toBe('developer')
      expect(service.resolveRole(['unknown'], config)).toBe('viewer')
    })
  })

  describe('findEnabledProviderForEmail', () => {
    it('matches the domain case-insensitively', async () => {
      ssoRepository.findAllEnabled.mockResolvedValue([makeRow()])
      const config = await service.findEnabledProviderForEmail('Person@ACME.com')
      expect(config?.id).toBe('cfg-1')
    })

    it('returns undefined when no provider claims the domain', async () => {
      ssoRepository.findAllEnabled.mockResolvedValue([makeRow()])
      expect(await service.findEnabledProviderForEmail('person@other.com')).toBeUndefined()
    })
  })

  describe('profileFromSaml', () => {
    const profileFromSaml = (p: Record<string, unknown>) =>
      (service as never as { profileFromSaml: Function }).profileFromSaml(p)

    it('extracts identity from standard claim keys', () => {
      const profile = profileFromSaml({
        nameID: 'user@acme.com',
        email: 'user@acme.com',
        firstName: 'Ada',
        lastName: 'Lovelace',
        groups: ['rushdb-admins', 'all']
      })
      expect(profile).toEqual({
        subject: 'user@acme.com',
        email: 'user@acme.com',
        firstName: 'Ada',
        lastName: 'Lovelace',
        groups: ['rushdb-admins', 'all']
      })
    })

    it('coerces a comma-separated groups string into an array', () => {
      const profile = profileFromSaml({ nameID: 'x', email: 'x@acme.com', groups: 'a, b ,c' })
      expect(profile.groups).toEqual(['a', 'b', 'c'])
    })

    it('falls back to NameID as the email (common SAML emailAddress format)', () => {
      const profile = profileFromSaml({ nameID: 'user@acme.com' })
      expect(profile.email).toBe('user@acme.com')
    })

    it('throws when neither an email claim nor NameID is present', () => {
      expect(() => profileFromSaml({ firstName: 'Nobody' })).toThrow()
    })
  })

  describe('AES state round-trip', () => {
    it('encrypts and decrypts opaque state', () => {
      const svc = service as never as { aesEncrypt: (s: string) => string; aesDecrypt: (s: string) => string }
      const payload = JSON.stringify({ configId: 'cfg-1', codeVerifier: 'abc', nonce: 'n' })
      const encrypted = svc.aesEncrypt(payload)
      expect(encrypted).not.toContain('configId')
      expect(svc.aesDecrypt(encrypted)).toBe(payload)
    })
  })

  describe('toPublic', () => {
    it('hides the OIDC secret but signals its presence and exposes SP URLs', () => {
      const config = makeConfig({
        type: 'oidc',
        oidcClientId: 'client',
        oidcClientSecretEncrypted: 'super-secret-ciphertext'
      })
      const out = service.toPublic(config)
      expect(out).not.toHaveProperty('oidcClientSecretEncrypted')
      expect(out.hasOidcClientSecret).toBe(true)
      expect(out.sp.oidcRedirectUri).toBe('https://api.rushdb.com/api/v1/auth/sso/oidc/cfg-1/callback')
      expect(out.sp.acsUrl).toBe('https://api.rushdb.com/api/v1/auth/sso/saml/cfg-1/acs')
    })
  })
})
