import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'
import { ApiModelProperty } from '@nestjs/swagger/dist/decorators/api-model-property.decorator'
import { IsNotEmpty } from 'class-validator'

export class InviteToWorkspaceDto {
  @IsNotEmpty()
  @ApiProperty()
  @ApiModelProperty()
  email: string

  @ApiPropertyOptional()
  projectIds: string[]
}
