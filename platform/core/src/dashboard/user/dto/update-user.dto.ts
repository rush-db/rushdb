import { ApiProperty } from '@nestjs/swagger'
import { ApiModelProperty } from '@nestjs/swagger/dist/decorators/api-model-property.decorator'

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
