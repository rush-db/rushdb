import { ArrayMinSize, IsArray, IsIn, IsNotEmpty, IsNumber, IsOptional, IsString } from 'class-validator'

export class InlineVectorEntryDto {
  @IsNotEmpty()
  @IsString()
  propertyName: string

  @IsArray()
  @ArrayMinSize(1)
  @IsNumber({}, { each: true })
  vector: number[]

  @IsOptional()
  @IsIn(['cosine', 'euclidean'])
  similarityFunction?: 'cosine' | 'euclidean'
}
