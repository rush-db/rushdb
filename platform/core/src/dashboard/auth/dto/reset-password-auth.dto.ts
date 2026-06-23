import { ApiProperty as ApiModelProperty } from '@nestjs/swagger'
import { IsNotEmpty } from 'class-validator'

export class ResetPasswordAuthDto {
  @IsNotEmpty()
  @ApiModelProperty()
  login: string

  @IsNotEmpty()
  @ApiModelProperty()
  password: string

  @IsNotEmpty()
  @ApiModelProperty()
  token: string
}
