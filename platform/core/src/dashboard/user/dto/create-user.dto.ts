import { ApiProperty } from '@nestjs/swagger'
import { ApiModelProperty } from '@nestjs/swagger/dist/decorators/api-model-property.decorator'
import { IsNotEmpty } from 'class-validator'

export class CreateUserDto {
  @IsNotEmpty()
  @ApiProperty()
  @ApiModelProperty()
  login: string

  @IsNotEmpty()
  @ApiProperty()
  @ApiModelProperty()
  password: string

  @ApiProperty()
  @ApiModelProperty()
  firstName: string

  @ApiProperty()
  @ApiModelProperty()
  lastName: string
}
