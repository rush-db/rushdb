import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'

import { TImportOptions, TImportJsonPayload } from '@/core/entity/import-export/import.types'

export class ImportCsvDto {
  @ApiPropertyOptional()
  parentId?: string

  @ApiProperty({ default: '' })
  payload: string

  @ApiPropertyOptional({ default: '' })
  label: string

  @ApiPropertyOptional({ default: {} })
  options?: TImportOptions
}
