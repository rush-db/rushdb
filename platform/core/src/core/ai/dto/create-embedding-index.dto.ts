import { ApiProperty } from '@nestjs/swagger'
import { IsBoolean, IsIn, IsInt, IsNotEmpty, IsOptional, IsString, Matches, Max, Min } from 'class-validator'

import {
  EMBEDDING_INDEX_SIMILARITY_FUNCTIONS,
  EMBEDDING_INDEX_SOURCE_TYPES
} from '@/core/ai/embedding-index.utils'

export class CreateEmbeddingIndexDto {
  @IsNotEmpty()
  @IsString()
  @ApiProperty({ example: 'description', description: 'Name of the property to embed' })
  propertyName: string

  @IsNotEmpty()
  @IsString()
  @Matches(/^[A-Za-z_][A-Za-z0-9_]*$/, {
    message:
      'label must be a valid Neo4j label (letters, digits, underscores; must start with a letter or underscore)'
  })
  @ApiProperty({ example: 'Book', description: 'Neo4j label to scope this index to (e.g. "Book", "Task")' })
  label: string

  @IsOptional()
  @IsString()
  @IsIn([...EMBEDDING_INDEX_SOURCE_TYPES])
  @ApiProperty({
    example: 'managed',
    required: false,
    enum: EMBEDDING_INDEX_SOURCE_TYPES,
    description: 'Whether RushDB generates embeddings or the client provides them'
  })
  sourceType?: 'managed' | 'external'

  @IsOptional()
  @IsBoolean()
  @ApiProperty({
    example: true,
    required: false,
    description:
      "Shorthand for sourceType: 'external'. When true, RushDB expects the client to provide vectors."
  })
  external?: boolean

  @IsOptional()
  @IsString()
  @IsIn([...EMBEDDING_INDEX_SIMILARITY_FUNCTIONS])
  @ApiProperty({
    example: 'cosine',
    required: false,
    enum: EMBEDDING_INDEX_SIMILARITY_FUNCTIONS,
    description: 'Similarity function for the vector index'
  })
  similarityFunction?: 'cosine' | 'euclidean'

  @IsOptional()
  @IsInt()
  @Min(1)
  @Max(4096)
  @ApiProperty({
    example: 1536,
    required: false,
    description: 'Vector dimensions. Defaults to the configured provider dimensions for managed indexes.'
  })
  dimensions?: number
}
