import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'

import { TImportOptions } from '@/core/entity/import-export/import.types'
import { PropertyDto } from '@/core/property/dto/property.dto'
import { TPropertyValue } from '@/core/property/property.types'

export class CreateEntityDto {
  @ApiProperty({ default: '' })
  label: string

  @ApiPropertyOptional({ type: [PropertyDto] })
  properties?: Array<PropertyDto>
}

export class CreateEntityDtoSimple {
  @ApiProperty({ default: '' })
  label: string

  @ApiPropertyOptional()
  payload?: Record<string, TPropertyValue>

  @ApiPropertyOptional()
  options?: Pick<TImportOptions, 'suggestTypes'>
}
