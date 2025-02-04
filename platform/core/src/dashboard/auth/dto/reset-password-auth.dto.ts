import { ApiModelProperty } from '@nestjs/swagger/dist/decorators/api-model-property.decorator'
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
