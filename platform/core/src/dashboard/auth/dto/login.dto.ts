import { ApiProperty } from '@nestjs/swagger'
import { ApiModelProperty } from '@nestjs/swagger/dist/decorators/api-model-property.decorator'
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
