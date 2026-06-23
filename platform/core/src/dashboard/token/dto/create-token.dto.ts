import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'
import { ApiProperty as ApiModelProperty } from '@nestjs/swagger'
import { IsIn, IsNotEmpty, IsOptional } from 'class-validator'

export class CreateTokenDto {
  @IsNotEmpty()
  @ApiProperty()
  @ApiModelProperty({ example: 'PhotoApp Token' })
  name: string

  @ApiPropertyOptional({ example: 'PhotoApp Token' })
  description: string

  @IsNotEmpty()
  @ApiProperty({ example: '30d' })
  expiration: string | '*'

  @IsOptional()
  @IsIn(['read', 'write'])
  @ApiPropertyOptional({
    enum: ['read', 'write'],
    default: 'write',
    description: 'Access level for the token'
  })
  level?: 'read' | 'write'

  // Internal: provenance metadata (set by OAuth token exchange, not exposed in public API)
  issuedBy?: 'dashboard' | 'oauth_exchange'
  consentId?: string
  scopes?: string
}

export class VerifyTokenDto {
  @IsNotEmpty()
  @ApiProperty()
  token: string
}
