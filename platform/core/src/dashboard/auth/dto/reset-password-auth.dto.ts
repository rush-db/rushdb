import { ApiModelProperty } from '@nestjs/swagger/dist/decorators/api-model-property.decorator'
import { IsNotEmpty } from 'class-validator'

export class ResetPasswordAuthDto {
  @IsNotEmpty()
  @ApiModelProperty()
  login: string

  @IsNotEmpty()
  @ApiModelProperty()
  userNewPassword: string

  @IsNotEmpty()
  @ApiModelProperty()
  userConfirmationToken: string
}
