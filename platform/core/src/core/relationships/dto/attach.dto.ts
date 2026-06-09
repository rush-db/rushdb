import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'

import { MaybeArray } from '@/core/common/types'
import { TRelationDirection } from '@/core/entity/entity.types'
import { RelationshipProperties } from '@/core/relationships/relationship-properties'

export class AttachDto {
  @ApiProperty()
  targetIds: MaybeArray<string>

  @ApiPropertyOptional()
  type?: string

  @ApiPropertyOptional()
  direction?: TRelationDirection

  @ApiPropertyOptional()
  properties?: RelationshipProperties
}
