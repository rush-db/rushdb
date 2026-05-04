import { Type } from 'class-transformer'
import { ArrayMinSize, IsArray, IsNotEmpty, IsNumber, IsString, ValidateNested } from 'class-validator'

class UpsertIndexVectorItemDto {
  @IsNotEmpty()
  @IsString()
  recordId: string

  @IsArray()
  @ArrayMinSize(1)
  @IsNumber({ allowInfinity: false, allowNaN: false }, { each: true })
  vector: number[]
}

export class UpsertIndexVectorsDto {
  @IsArray()
  @ArrayMinSize(1)
  @ValidateNested({ each: true })
  @Type(() => UpsertIndexVectorItemDto)
  items: UpsertIndexVectorItemDto[]
}
