import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'
import { IsIn, IsNotEmpty, IsObject, IsOptional, IsString } from 'class-validator'

import { SAVED_QUERY_SEARCH_MODES, SavedQuerySearchMode } from '@/dashboard/saved-query/saved-query.types'

export class CreateSavedQueryDto {
  @IsNotEmpty()
  @IsString()
  @ApiProperty({ example: 'Top planets by battles' })
  name: string

  @IsIn(SAVED_QUERY_SEARCH_MODES)
  @ApiProperty({ enum: SAVED_QUERY_SEARCH_MODES })
  searchMode: SavedQuerySearchMode

  @IsObject()
  @ApiProperty({
    example: {
      labels: ['PLANET'],
      orderBy: { fights: 'desc' }
    }
  })
  searchQuery: Record<string, unknown>

  @IsOptional()
  @IsString()
  @ApiPropertyOptional({ example: 'what planet held most fights?' })
  prompt?: string

  @IsOptional()
  @IsString()
  @ApiPropertyOptional()
  semanticIndexId?: string
}

export class UpdateSavedQueryDto {
  @IsOptional()
  @IsString()
  @ApiPropertyOptional()
  name?: string

  @IsOptional()
  @IsIn(SAVED_QUERY_SEARCH_MODES)
  @ApiPropertyOptional({ enum: SAVED_QUERY_SEARCH_MODES })
  searchMode?: SavedQuerySearchMode

  @IsOptional()
  @IsObject()
  @ApiPropertyOptional()
  searchQuery?: Record<string, unknown>

  @IsOptional()
  @IsString()
  @ApiPropertyOptional()
  prompt?: string

  @IsOptional()
  @IsString()
  @ApiPropertyOptional()
  semanticIndexId?: string
}
