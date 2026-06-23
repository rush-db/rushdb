import { ApiProperty } from '@nestjs/swagger'
import { ApiProperty as ApiModelProperty } from '@nestjs/swagger'
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
