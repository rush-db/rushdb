import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'
import { IsNotEmpty, IsOptional, IsString } from 'class-validator'

export class TokenRequestDto {
  @IsNotEmpty()
  @IsString()
  @ApiProperty({
    example: 'authorization_code',
    description:
      'OAuth grant type. Supported: authorization_code, urn:ietf:params:oauth:grant-type:token-exchange'
  })
  grant_type: string

  // --- authorization_code fields ---

  @IsOptional()
  @IsString()
  @ApiPropertyOptional({ example: 'oa_code_...' })
  code?: string

  @IsOptional()
  @IsString()
  @ApiPropertyOptional({ example: 'https://chatgpt.com/connector_platform_oauth_redirect' })
  redirect_uri?: string

  @IsOptional()
  @IsString()
  @ApiPropertyOptional({ example: 'oauthc_...' })
  client_id?: string

  @IsOptional()
  @IsString()
  @ApiPropertyOptional({ description: 'PKCE code verifier (RFC 7636)' })
  code_verifier?: string

  @IsOptional()
  @IsString()
  @ApiPropertyOptional({ example: 'https://api.rushdb.com' })
  resource?: string

  // --- token-exchange fields (RFC 8693) ---

  @IsOptional()
  @IsString()
  @ApiPropertyOptional()
  subject_token?: string

  @IsOptional()
  @IsString()
  @ApiPropertyOptional({ example: 'urn:ietf:params:oauth:token-type:access_token' })
  subject_token_type?: string

  @IsOptional()
  @IsString()
  @ApiPropertyOptional({ example: 'urn:ietf:params:oauth:token-type:access_token' })
  requested_token_type?: string

  @IsOptional()
  @IsString()
  @ApiPropertyOptional({ example: 'proj_1', description: 'Target project for token exchange' })
  project_id?: string

  // --- refresh_token fields (RFC 6749 §6) ---

  @IsOptional()
  @IsString()
  @ApiPropertyOptional({ description: 'Opaque refresh token issued during authorization_code exchange' })
  refresh_token?: string
}
