import { ApiPropertyOptional } from '@nestjs/swagger'

import { TImportOptions } from '@/core/entity/import-export/import.types'
import { PropertyDto } from '@/core/property/dto/property.dto'
import { TPropertyValue } from '@/core/property/property.types'

export class EditEntityDto {
  @ApiPropertyOptional({ default: '' })
  label: string

  @ApiPropertyOptional({ type: [PropertyDto] })
  properties?: Array<PropertyDto>
}

export class EditEntityDtoSimple {
  @ApiPropertyOptional({ default: '' })
  label: string

  @ApiPropertyOptional()
  payload?: Record<string, TPropertyValue>

  @ApiPropertyOptional()
  options?: Pick<TImportOptions, 'suggestTypes'>
}
