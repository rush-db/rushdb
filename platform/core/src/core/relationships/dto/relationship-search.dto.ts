import { ApiPropertyOptional } from '@nestjs/swagger'

import { Where } from '@/core/common/types'
import { SearchDto } from '@/core/search/dto/search.dto'

export class RelationshipEndpointSearchDto {
  @ApiPropertyOptional({ default: [] })
  labels?: string[]

  @ApiPropertyOptional({ default: {} })
  where?: Where
}

export class RelationshipSearchDto extends SearchDto {
  @ApiPropertyOptional()
  source?: RelationshipEndpointSearchDto

  @ApiPropertyOptional()
  target?: RelationshipEndpointSearchDto
}
