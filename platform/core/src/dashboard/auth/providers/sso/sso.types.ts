import { TUserRoles } from '@/dashboard/user/model/user.interface'

import type { WorkspaceIdentityProviderRow } from '@/database/sql/schema/types'

export type TIdpType = 'saml' | 'oidc'

/**
 * Identity provider config with JSON columns parsed into their runtime shapes.
 */
export type TIdpConfig = Omit<WorkspaceIdentityProviderRow, 'domains' | 'groupMappings'> & {
  domains: string[]
  groupMappings: Record<string, TUserRoles> | null
}

/**
 * Normalized identity extracted from a verified SAML assertion or OIDC ID token.
 * `subject` is the stable external identifier (SAML NameID / OIDC sub).
 */
export type TSsoProfile = {
  subject: string
  email: string
  firstName?: string
  lastName?: string
  groups: string[]
}

/**
 * Transient state carried through the OIDC redirect round-trip. Encrypted with
 * RUSHDB_AES_256_ENCRYPTION_KEY so the PKCE verifier never leaves our control
 * in cleartext, even though the value passes through the user agent and IdP.
 */
export type TOidcState = {
  configId: string
  codeVerifier: string
  nonce: string
  redirectUrl?: string
}

/**
 * Transient state carried through the SAML redirect as RelayState.
 */
export type TSamlRelayState = {
  configId: string
  redirectUrl?: string
}
