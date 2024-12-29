import { ApiPropertyOptional } from '@nestjs/swagger'
import { ApiModelProperty } from '@nestjs/swagger/dist/decorators/api-model-property.decorator'

import { TPropertyValue } from '@/core/property/property.types'

export class UpdatePropertyValueDto {
  @ApiPropertyOptional()
  @ApiModelProperty()
  entityIds?: string[]

  @ApiPropertyOptional()
  @ApiModelProperty({ example: 5 })
  newValue?: TPropertyValue
}
