import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'

import { MaybeArray } from '@/core/common/types'
import { TRelationDirection } from '@/core/entity/entity.types'

export class DetachDto {
  @ApiProperty()
  targetIds: MaybeArray<string>

  @ApiPropertyOptional()
  typeOrTypes?: MaybeArray<string>

  @ApiPropertyOptional()
  direction?: TRelationDirection
}
