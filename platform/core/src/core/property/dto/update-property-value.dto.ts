import { ApiPropertyOptional } from '@nestjs/swagger'
import { ApiProperty as ApiModelProperty } from '@nestjs/swagger'

import { TPropertyValue } from '@/core/property/property.types'

export class UpdatePropertyValueDto {
  @ApiPropertyOptional()
  @ApiModelProperty()
  entityIds?: string[]

  @ApiPropertyOptional()
  @ApiModelProperty({ example: 5 })
  newValue?: TPropertyValue
}
