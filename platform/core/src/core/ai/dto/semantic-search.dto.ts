import {
  ArrayMinSize,
  IsArray,
  IsIn,
  IsNotEmpty,
  IsNumber,
  IsOptional,
  IsString,
  Max,
  Min
} from 'class-validator'
import { Where } from '@/core/common/types'

import {
  EMBEDDING_INDEX_SIMILARITY_FUNCTIONS,
  EMBEDDING_INDEX_SOURCE_TYPES
} from '@/core/ai/embedding-index.utils'

export class SemanticSearchDto {
  /** Name of the property whose embedding index to query */
  @IsNotEmpty()
  @IsString()
  propertyName: string

  /** Free-text query that will be embedded and used for semantic similarity search */
  @IsOptional()
  @IsString()
  query?: string

  @IsOptional()
  @IsArray()
  @ArrayMinSize(1)
  @IsNumber({}, { each: true })
  queryVector?: number[]

  /**
   * One or more Neo4j labels to scope the search.
   * The first label is used to resolve which embedding index to use.
   * Required — always provide at least one label.
   */
  @IsNotEmpty()
  @IsArray()
  @ArrayMinSize(1)
  @IsString({ each: true })
  labels: string[]

  @IsOptional()
  @IsString()
  @IsIn([...EMBEDDING_INDEX_SOURCE_TYPES])
  sourceType?: 'managed' | 'external'

  @IsOptional()
  @IsString()
  @IsIn([...EMBEDDING_INDEX_SIMILARITY_FUNCTIONS])
  similarityFunction?: 'cosine' | 'euclidean'

  @IsOptional()
  @IsNumber()
  @Min(1)
  @Max(4096)
  dimensions?: number

  /**
   * Optional Cypher WHERE filter applied before cosine scoring.
   * Supports all standard RushDB filter operators.
   */
  where?: Where

  /** Number of results to skip for pagination (default 0) */
  skip?: number

  /** Maximum number of results to return (default 20) */
  limit?: number
}
