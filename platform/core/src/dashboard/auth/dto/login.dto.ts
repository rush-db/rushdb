import { ApiProperty } from '@nestjs/swagger'
import { ApiProperty as ApiModelProperty } from '@nestjs/swagger'
import { IsNotEmpty } from 'class-validator'

export class LoginDto {
  @IsNotEmpty()
  @ApiProperty()
  @ApiModelProperty()
  login: string

  @IsNotEmpty()
  @ApiProperty()
  @ApiModelProperty()
  password: string
}
