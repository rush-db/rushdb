import { ApiExtraModels, ApiProperty, ApiPropertyOptional, getSchemaPath } from '@nestjs/swagger'

import { InlineVectorEntryDto } from '@/core/ai/dto/inline-vector-entry.dto'
import { TImportOptions, TImportCsvParseConfig } from '@/core/entity/import-export/import.types'

@ApiExtraModels(InlineVectorEntryDto)
export class ImportCsvDto {
  @ApiPropertyOptional()
  parentId?: string

  @ApiProperty({ default: '' })
  data: string

  @ApiPropertyOptional({ default: '' })
  label: string

  @ApiPropertyOptional({ default: {} })
  options?: TImportOptions

  @ApiPropertyOptional({
    description: 'CSV parse config (allowed subset)',
    default: {
      delimiter: ',',
      header: true,
      skipEmptyLines: true
    }
  })
  parseConfig?: TImportCsvParseConfig

  @ApiPropertyOptional({
    description:
      'Per-row inline vectors for external embedding indexes. vectors[i] is applied to CSV row i (0-based, after header).',
    type: 'array',
    items: { type: 'array', items: { $ref: getSchemaPath(InlineVectorEntryDto) } }
  })
  vectors?: InlineVectorEntryDto[][]
}
