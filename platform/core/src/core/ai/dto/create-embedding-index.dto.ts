import { ApiProperty } from '@nestjs/swagger'
import { IsNotEmpty, IsString, Matches } from 'class-validator'

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
}
