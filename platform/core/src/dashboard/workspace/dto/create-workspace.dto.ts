import { ApiProperty } from '@nestjs/swagger'
import { ApiProperty as ApiModelProperty } from '@nestjs/swagger'
import { IsNotEmpty } from 'class-validator'

export class CreateWorkspaceDto {
  @IsNotEmpty()
  @ApiProperty()
  @ApiModelProperty()
  name: string
}
