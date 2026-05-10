import { Controller, Get } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { ApiOperation, ApiTags } from '@nestjs/swagger'

import { createPublicKey } from 'node:crypto'

@ApiTags('OAuth Discovery')
@Controller()
export class WellKnownController {
  constructor(private readonly configService: ConfigService) {}

  private decodePem(value?: string): string | undefined {
    if (!value) {
      return undefined
    }
    return value.replace(/\\n/g, '\n')
  }

  private decodeBase64Pem(value?: string): string | undefined {
    if (!value) {
      return undefined
    }
    try {
      return this.decodePem(Buffer.from(value, 'base64').toString('utf8'))
    } catch {
      return undefined
    }
  }

  private get jwtKid(): string {
    return this.configService.get<string>('RUSHDB_JWT_KID') || 'rushdb-mcp-rs256'
  }

  private get jwtPublicPem(): string | undefined {
    const explicitPublic =
      this.decodePem(this.configService.get<string>('RUSHDB_JWT_PUBLIC_KEY')) ||
      this.decodeBase64Pem(this.configService.get<string>('RUSHDB_JWT_PUBLIC_KEY_BASE64'))
    if (explicitPublic) {
      return explicitPublic
    }

    const privatePem =
      this.decodePem(this.configService.get<string>('RUSHDB_JWT_PRIVATE_KEY')) ||
      this.decodeBase64Pem(this.configService.get<string>('RUSHDB_JWT_PRIVATE_KEY_BASE64'))
    if (!privatePem) {
      return undefined
    }

    try {
      return createPublicKey(privatePem).export({ format: 'pem', type: 'spki' }).toString()
    } catch {
      return undefined
    }
  }

  private get issuer(): string {
    return this.configService.get<string>('RUSHDB_OAUTH_ISSUER') || 'https://api.rushdb.com'
  }

  @Get('.well-known/oauth-authorization-server')
  @ApiOperation({ summary: 'OAuth 2.0 Authorization Server Metadata (RFC 8414)' })
  getAuthorizationServerMetadata() {
    return {
      issuer: this.issuer,
      authorization_endpoint: `${this.issuer}/oauth/authorize`,
      token_endpoint: `${this.issuer}/oauth/token`,
      registration_endpoint: `${this.issuer}/oauth/register`,
      jwks_uri: `${this.issuer}/.well-known/jwks.json`,
      scopes_supported: ['records:read', 'records:write'],
      response_types_supported: ['code'],
      grant_types_supported: ['authorization_code', 'urn:ietf:params:oauth:grant-type:token-exchange'],
      code_challenge_methods_supported: ['S256'],
      token_endpoint_auth_methods_supported: ['none'],
      service_documentation: 'https://docs.rushdb.com'
    }
  }

  @Get('.well-known/oauth-protected-resource')
  @ApiOperation({ summary: 'OAuth 2.0 Protected Resource Metadata' })
  getProtectedResourceMetadata() {
    return {
      resource: this.issuer,
      authorization_servers: [this.issuer],
      scopes_supported: ['records:read', 'records:write'],
      bearer_methods_supported: ['header'],
      resource_documentation: 'https://docs.rushdb.com/mcp-server'
    }
  }

  @Get('.well-known/jwks.json')
  @ApiOperation({ summary: 'JSON Web Key Set (JWKS) for OAuth access-token verification' })
  getJwks() {
    const publicPem = this.jwtPublicPem
    if (!publicPem) {
      return { keys: [] }
    }

    try {
      const jwk = createPublicKey(publicPem).export({ format: 'jwk' }) as Record<string, unknown>
      return {
        keys: [
          {
            ...jwk,
            use: 'sig',
            alg: 'RS256',
            kid: this.jwtKid
          }
        ]
      }
    } catch {
      return { keys: [] }
    }
  }
}
