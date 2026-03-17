import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'
import { IsNotEmpty, IsOptional, IsString } from 'class-validator'

export class AuthorizeAcceptDto {
  @IsNotEmpty()
  @IsString()
  @ApiProperty({ example: 'oa_req_...' })
  auth_request_id: string

  @IsNotEmpty()
  @IsString()
  @ApiProperty({ example: 'proj_019c90d5-...' })
  project_id: string

  @IsOptional()
  @IsString()
  @ApiPropertyOptional({
    example: 'records:read',
    description: 'Space-separated list of scopes to grant. Defaults to all requested scopes.'
  })
  scope?: string
}
