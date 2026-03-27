import { IsArray, IsIn, IsNumber, IsOptional, IsString } from 'class-validator'

export class InlineVectorEntryDto {
  @IsString()
  propertyName: string

  @IsArray()
  @IsNumber({}, { each: true })
  vector: number[]

  @IsOptional()
  @IsIn(['cosine', 'euclidean'])
  similarityFunction?: 'cosine' | 'euclidean'
}
