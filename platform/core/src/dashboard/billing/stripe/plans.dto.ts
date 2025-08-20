import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'
import { ApiModelProperty } from '@nestjs/swagger/dist/decorators/api-model-property.decorator'
import { IsNotEmpty } from 'class-validator'

export class PlansDto {
  @IsNotEmpty()
  @ApiProperty()
  @ApiModelProperty()
  priceId: string

  @ApiModelProperty()
  @ApiPropertyOptional()
  projectId?: string

  @IsNotEmpty()
  @ApiProperty()
  @ApiModelProperty({ example: 'http://localhost:3005/' })
  returnUrl: string
}
