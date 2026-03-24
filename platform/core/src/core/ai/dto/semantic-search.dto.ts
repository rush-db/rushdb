import { IsArray, IsNotEmpty, ArrayMinSize, IsString } from 'class-validator'
import { Where } from '@/core/common/types'

export class SemanticSearchDto {
  /** Name of the property whose embedding index to query */
  propertyName: string

  /** Free-text query that will be embedded and used for semantic similarity search */
  query: string

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
