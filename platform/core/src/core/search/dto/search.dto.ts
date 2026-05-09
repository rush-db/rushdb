import { ApiPropertyOptional } from '@nestjs/swagger'

import { Aggregate, SelectExprMap, Where } from '@/core/common/types'
import { TSearchSort } from '@/core/search/search.types'

export class SearchDto {
  @ApiPropertyOptional({ default: 100 })
  limit?: number

  @ApiPropertyOptional({ default: 0 })
  skip?: number

  @ApiPropertyOptional()
  orderBy?: TSearchSort

  @ApiPropertyOptional({ default: {} })
  where?: Where

  @ApiPropertyOptional({ default: {} })
  select?: SelectExprMap

  /** @deprecated Use `select` instead. To be removed in a future major version. */
  @ApiPropertyOptional({ default: {}, deprecated: true })
  aggregate?: Aggregate

  @ApiPropertyOptional({ default: [] })
  labels?: string[]

  @ApiPropertyOptional({ default: [] })
  groupBy?: string[]
}
