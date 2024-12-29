import { ApiPropertyOptional } from '@nestjs/swagger'
import { ApiModelProperty } from '@nestjs/swagger/dist/decorators/api-model-property.decorator'
export class UpdatePropertyDto {
  @ApiPropertyOptional()
  @ApiModelProperty({ example: 'NewPropertyName' })
  name?: string

  @ApiPropertyOptional()
  @ApiModelProperty({ example: 'string' })
  type?: 'string'
}
