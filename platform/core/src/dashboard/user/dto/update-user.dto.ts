import { ApiProperty } from '@nestjs/swagger'
import { ApiProperty as ApiModelProperty } from '@nestjs/swagger'

// @TODO: Optional Props and more options
export class UpdateUserDto {
  @ApiProperty()
  @ApiModelProperty()
  firstName?: string

  @ApiModelProperty()
  @ApiProperty()
  lastName?: string

  @ApiModelProperty()
  @ApiProperty()
  settings?: string
}
