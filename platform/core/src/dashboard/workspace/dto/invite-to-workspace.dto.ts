import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'
import { ApiProperty as ApiModelProperty } from '@nestjs/swagger'
import { IsNotEmpty } from 'class-validator'

export class InviteToWorkspaceDto {
  @IsNotEmpty()
  @ApiProperty()
  @ApiModelProperty()
  email: string

  @ApiPropertyOptional()
  projectIds: string[]
}
