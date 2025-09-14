import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'

import { TImportOptions, TImportCsvParseConfig } from '@/core/entity/import-export/import.types'

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
}
