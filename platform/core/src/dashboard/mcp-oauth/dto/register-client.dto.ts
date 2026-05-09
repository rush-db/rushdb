import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'
import { IsArray, IsIn, IsNotEmpty, IsOptional, IsString, IsUrl } from 'class-validator'

export class RegisterClientDto {
  @IsArray()
  @IsNotEmpty()
  @ApiProperty({ type: [String], example: ['https://chatgpt.com/connector_platform_oauth_redirect'] })
  redirect_uris: string[]

  @IsOptional()
  @IsString()
  @ApiPropertyOptional({ example: 'ChatGPT' })
  client_name?: string

  @IsOptional()
  @IsIn(['none'])
  @ApiPropertyOptional({ enum: ['none'], default: 'none' })
  token_endpoint_auth_method?: 'none'

  @IsOptional()
  @IsIn(['web', 'native'])
  @ApiPropertyOptional({ enum: ['web', 'native'], default: 'web' })
  application_type?: 'web' | 'native'
}
