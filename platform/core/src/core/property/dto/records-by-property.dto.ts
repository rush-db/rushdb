import { ApiPropertyOptional } from '@nestjs/swagger'

import { TPropertyType } from '@/core/property/property.types'
import { SearchDto } from '@/core/search/dto/search.dto'

export class RecordsByPropertyDto extends SearchDto {
  @ApiPropertyOptional()
  forceType?: TPropertyType
}
