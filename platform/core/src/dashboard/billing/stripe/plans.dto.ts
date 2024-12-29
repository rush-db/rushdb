import { ApiProperty } from '@nestjs/swagger'
import { ApiModelProperty } from '@nestjs/swagger/dist/decorators/api-model-property.decorator'
import { IsNotEmpty } from 'class-validator'

export class PlansDto {
  @IsNotEmpty()
  @ApiProperty()
  @ApiModelProperty({ example: 'pro' })
  id: 'pro'

  @IsNotEmpty()
  @ApiProperty()
  @ApiModelProperty({ example: 'month' })
  period: 'month' | 'annual'

  @IsNotEmpty()
  @ApiProperty()
  @ApiModelProperty({ example: 'http://localhost:3005/' })
  returnUrl: string
}
