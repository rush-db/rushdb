import { Controller, Get } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { ApiOperation, ApiTags } from '@nestjs/swagger'

@ApiTags('OAuth Discovery')
@Controller()
export class WellKnownController {
  constructor(private readonly configService: ConfigService) {}

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
  @ApiOperation({ summary: 'JSON Web Key Set (JWKS) — placeholder for RS256 upgrade path' })
  getJwks() {
    // Placeholder: returns empty keyset. When upgrading from HS256 to RS256,
    // replace with the actual public key set.
    return { keys: [] }
  }
}
