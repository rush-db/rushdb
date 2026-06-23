import type { WorkspaceRole } from '~/features/workspaces/types'

export type IdpType = 'saml' | 'oidc'

export interface SsoServiceProviderInfo {
  acsUrl: string
  entityId: string
  metadataUrl: string
  oidcRedirectUri: string
}

export interface SsoConfig {
  id: string
  workspaceId: string
  type: IdpType
  enabled: boolean
  enforced: boolean
  domains: string[]
  defaultRole: WorkspaceRole
  groupMappings: Record<string, WorkspaceRole> | null
  samlEntityId: string | null
  samlSsoUrl: string | null
  samlCertificate: string | null
  oidcIssuer: string | null
  oidcClientId: string | null
  hasOidcClientSecret: boolean
  sp: SsoServiceProviderInfo
  createdAt: string
  updatedAt: string
}

export interface UpsertSsoConfigPayload {
  type: IdpType
  enabled?: boolean
  enforced?: boolean
  domains: string[]
  defaultRole?: WorkspaceRole
  groupMappings?: Record<string, WorkspaceRole>
  samlEntityId?: string
  samlSsoUrl?: string
  samlCertificate?: string
  oidcIssuer?: string
  oidcClientId?: string
  oidcClientSecret?: string
}

export interface SsoDiscoverResponse {
  ssoAvailable: boolean
  type?: IdpType
  enforced?: boolean
  loginUrl?: string
}
