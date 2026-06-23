import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'
import { IsArray, IsBoolean, IsIn, IsOptional, IsString } from 'class-validator'

import { USER_ROLE_LIST } from '@/dashboard/user/interfaces/user.constants'
import { TUserRoles } from '@/dashboard/user/model/user.interface'

export class UpsertSsoConfigDto {
  @IsIn(['saml', 'oidc'])
  @ApiProperty({ enum: ['saml', 'oidc'] })
  type: 'saml' | 'oidc'

  @IsOptional()
  @IsBoolean()
  @ApiPropertyOptional()
  enabled?: boolean

  @IsOptional()
  @IsBoolean()
  @ApiPropertyOptional()
  enforced?: boolean

  @IsArray()
  @IsString({ each: true })
  @ApiProperty({ type: [String] })
  domains: string[]

  @IsOptional()
  @IsIn(USER_ROLE_LIST as unknown as string[])
  @ApiPropertyOptional({ enum: USER_ROLE_LIST })
  defaultRole?: TUserRoles

  @IsOptional()
  @ApiPropertyOptional({ description: 'Map of IdP group/claim value to RushDB role' })
  groupMappings?: Record<string, TUserRoles>

  // SAML
  @IsOptional()
  @IsString()
  @ApiPropertyOptional()
  samlEntityId?: string

  @IsOptional()
  @IsString()
  @ApiPropertyOptional()
  samlSsoUrl?: string

  @IsOptional()
  @IsString()
  @ApiPropertyOptional()
  samlCertificate?: string

  // OIDC
  @IsOptional()
  @IsString()
  @ApiPropertyOptional()
  oidcIssuer?: string

  @IsOptional()
  @IsString()
  @ApiPropertyOptional()
  oidcClientId?: string

  /** Plaintext secret from the admin form; stored encrypted. Omit to keep existing. */
  @IsOptional()
  @IsString()
  @ApiPropertyOptional()
  oidcClientSecret?: string
}
