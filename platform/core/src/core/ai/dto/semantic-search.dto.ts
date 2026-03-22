import { IsArray, IsNotEmpty, ArrayMinSize, IsString } from 'class-validator'
import { Where } from '@/core/common/types'

export class SemanticSearchDto {
  /** Name of the property whose embedding index to query */
  propertyName: string

  /** Free-text query that will be embedded and used for ANN search */
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
   * Optional Cypher WHERE prefilter. When present, the search switches from ANN to exact
   * (ENN) mode: candidates are first narrowed via MATCH/WHERE, then scored with
   * vector.similarity.cosine(). Supports all standard RushDB filter operators.
   */
  where?: Where

  /** Maximum number of candidate results to retrieve from the ANN index (default 20, ignored in prefilter mode) */
  topK?: number

  /** Number of results to skip for pagination (default 0) */
  skip?: number

  /** Maximum number of results to return (default 20) */
  limit?: number
}
