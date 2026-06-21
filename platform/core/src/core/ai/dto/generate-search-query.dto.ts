import { ApiPropertyOptional } from '@nestjs/swagger'

import { SearchDto } from '@/core/search/dto/search.dto'

export class GenerateSearchQueryDto {
  @ApiPropertyOptional()
  prompt: string

  @ApiPropertyOptional()
  currentQuery?: SearchDto
}
