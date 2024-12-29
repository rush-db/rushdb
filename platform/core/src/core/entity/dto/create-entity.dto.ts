import { ApiPropertyOptional } from '@nestjs/swagger'

import { TRelationDirection } from '@/core/entity/entity.types'
import { TImportOptions } from '@/core/entity/import-export/import.types'
import { PropertyDto } from '@/core/property/dto/property.dto'
import { TPropertyValue } from '@/core/property/property.types'

export class CreateEntityDto {
  @ApiPropertyOptional({ default: '' })
  label: string

  @ApiPropertyOptional({ type: [PropertyDto] })
  properties?: Array<PropertyDto>
}

export class CreateEntityDtoSimple {
  @ApiPropertyOptional({ default: '' })
  label: string

  @ApiPropertyOptional()
  payload?: Record<string, TPropertyValue>

  @ApiPropertyOptional()
  options?: Pick<TImportOptions, 'suggestTypes'>
}
