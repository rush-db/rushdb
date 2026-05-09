// Copyright Collect Software, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import { createDbForToken } from './db.js'
import type { RequestContext } from './db.js'

const RUSHDB_API_URL = process.env.RUSHDB_API_URL || 'https://api.rushdb.com/api/v1'
const RUSHDB_OAUTH_ISSUER = process.env.RUSHDB_OAUTH_ISSUER || 'https://api.rushdb.com'
const TOKEN_EXCHANGE_GRANT = 'urn:ietf:params:oauth:grant-type:token-exchange'

/**
 * Error class for authentication failures in HTTP mode.
 */
export class AuthError extends Error {
  constructor(
    public readonly code: string,
    message: string
  ) {
    super(message)
    this.name = 'AuthError'
  }
}

/**
 * Returns whether a string looks like a JWT (three base64url segments separated by dots).
 */
export function isJwt(token: string): boolean {
  const parts = token.split('.')
  return parts.length === 3
}

/**
 * Builds an MCP-compatible authentication error response.
 *
 * ChatGPT reads `_meta["mcp/www_authenticate"]` to trigger the Account Linking UI.
 * The value MUST be an array of WWW-Authenticate Bearer challenge strings and MUST
 * include both `error` and `error_description` parameters, otherwise ChatGPT will
 * not surface the linking UI.
 *
 * Spec reference:
 *   https://developers.openai.com/apps-sdk/build/auth#triggering-authentication-ui
 */
export function makeMcpAuthError(
  resourceMetadataUrl: string,
  error = 'insufficient_scope',
  errorDescription = 'Link your RushDB account to continue.'
) {
  const challenge = `Bearer resource_metadata="${resourceMetadataUrl}", error="${error}", error_description="${errorDescription}"`
  return {
    _meta: {
      'mcp/www_authenticate': [challenge]
    },
    content: [
      {
        type: 'text',
        text: `Authentication required: ${errorDescription}`
      }
    ],
    isError: true
  }
}

/**
 * Resolves a per-request execution context from an incoming bearer token.
 *
 * Supports two token shapes:
 *  - **JWT** (3-dot format): verifies against the OAuth issuer's JWKS, then
 *    performs token exchange to obtain a short-lived project API token.
 *  - **Opaque API key**: used directly as the RushDB project token (API-key
 *    compatibility mode — equivalent to STDIO mode but over HTTP).
 *
 * @param bearerToken  The raw token value after stripping "Bearer ".
 * @param projectId    Optional project ID override (from ?project_id= query param).
 */
export async function resolveRequestContext(
  bearerToken: string,
  projectId?: string
): Promise<RequestContext> {
  if (isJwt(bearerToken)) {
    return resolveJwtContext(bearerToken, projectId)
  }
  // Opaque API key — direct compatibility mode
  return resolveApiKeyContext(bearerToken)
}

async function resolveJwtContext(jwt: string, projectId?: string): Promise<RequestContext> {
  // Dynamic import of jose to avoid bundling issues when unused in stdio mode
  const { jwtVerify, createRemoteJWKSet } = await import('jose')

  let payload: any
  try {
    // Try JWKS verification first (RS256 — production path)
    const JWKS = createRemoteJWKSet(new URL(`${RUSHDB_OAUTH_ISSUER}/.well-known/jwks.json`))
    const result = await jwtVerify(jwt, JWKS, {
      issuer: RUSHDB_OAUTH_ISSUER,
      audience: RUSHDB_OAUTH_ISSUER
    })
    payload = result.payload
  } catch (_jwksError) {
    // Fallback: HS256 with shared secret (MVP / self-hosted path)
    // The shared secret is the RUSHDB_AES_256_ENCRYPTION_KEY env var, same as platform/core
    const secret = process.env.RUSHDB_AES_256_ENCRYPTION_KEY
    if (!secret) {
      throw new AuthError('invalid_token', 'Cannot verify JWT: no JWKS and no shared secret configured')
    }
    try {
      const { jwtVerify: jwtVerifyHmac } = await import('jose')
      const encoder = new TextEncoder()
      const result = await jwtVerifyHmac(jwt, encoder.encode(secret))
      payload = result.payload
    } catch (e) {
      throw new AuthError('invalid_token', `JWT verification failed: ${(e as Error).message}`)
    }
  }

  const projectIdFromToken: string | undefined = payload.project_id as string | undefined
  const scopes: string[] = ((payload.scope as string) || '').split(' ').filter(Boolean)
  const effectiveProjectId = projectId || projectIdFromToken

  if (!effectiveProjectId) {
    throw new AuthError('no_project', 'No project_id specified and none in token')
  }
  if (projectIdFromToken && effectiveProjectId !== projectIdFromToken) {
    throw new AuthError(
      'project_not_authorized',
      `Project ${effectiveProjectId} is not authorized by this token`
    )
  }

  // Token exchange: get a short-lived project API token
  const projectToken = await performTokenExchange(jwt, effectiveProjectId)
  const db = createDbForToken(projectToken)

  return {
    db,
    userId: payload.sub as string,
    scopes,
    projectId: effectiveProjectId
  }
}

async function resolveApiKeyContext(apiKey: string): Promise<RequestContext> {
  const db = createDbForToken(apiKey)
  return {
    db,
    scopes: ['records:read', 'records:write']
  }
}

/**
 * Calls the platform/core token exchange endpoint to mint a short-lived
 * project-scoped API token from an OAuth JWT.
 */
async function performTokenExchange(subjectToken: string, projectId: string): Promise<string> {
  const tokenEndpoint = `${RUSHDB_OAUTH_ISSUER}/oauth/token`

  const body = new URLSearchParams({
    grant_type: TOKEN_EXCHANGE_GRANT,
    subject_token: subjectToken,
    subject_token_type: 'urn:ietf:params:oauth:token-type:access_token',
    requested_token_type: 'urn:ietf:params:oauth:token-type:access_token',
    project_id: projectId,
    resource: RUSHDB_API_URL
  })

  const response = await fetch(tokenEndpoint, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: body.toString()
  })

  if (!response.ok) {
    const errorText = await response.text().catch(() => 'unknown error')
    throw new AuthError('token_exchange_failed', `Token exchange failed (${response.status}): ${errorText}`)
  }

  const data: any = await response.json()
  if (!data.access_token) {
    throw new AuthError('token_exchange_failed', 'Token exchange response missing access_token')
  }

  return data.access_token
}
