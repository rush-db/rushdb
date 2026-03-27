import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'
import { Type } from 'class-transformer'
import { IsArray, IsOptional, ValidateNested } from 'class-validator'

import { InlineVectorEntryDto } from '@/core/ai/dto/inline-vector-entry.dto'
import { TCreateRecordSchema } from '@/core/entity/entity.types'
import { TImportOptions } from '@/core/entity/import-export/import.types'
import { PropertyDto } from '@/core/property/dto/property.dto'
import { TPropertyValue } from '@/core/property/property.types'

export class CreateEntityDto {
  @ApiProperty({ default: '' })
  label: string

  @ApiPropertyOptional({ type: [PropertyDto] })
  properties?: Array<PropertyDto>

  @ApiPropertyOptional()
  options?: Omit<TImportOptions, 'returnResult'>

  @IsOptional()
  @IsArray()
  @ValidateNested({ each: true })
  @Type(() => InlineVectorEntryDto)
  @ApiPropertyOptional({
    type: [InlineVectorEntryDto],
    description: 'Inline vectors for external embedding indexes'
  })
  vectors?: InlineVectorEntryDto[]
}

export class CreateEntityDtoSimple {
  @ApiProperty({ default: '' })
  label: string

  @ApiProperty()
  data: Record<string, TPropertyValue>

  @ApiPropertyOptional()
  options?: Omit<TImportOptions, 'returnResult'>

  @ApiPropertyOptional()
  schema?: TCreateRecordSchema

  @IsOptional()
  @IsArray()
  @ValidateNested({ each: true })
  @Type(() => InlineVectorEntryDto)
  @ApiPropertyOptional({
    type: [InlineVectorEntryDto],
    description: 'Inline vectors for external embedding indexes'
  })
  vectors?: InlineVectorEntryDto[]
}
