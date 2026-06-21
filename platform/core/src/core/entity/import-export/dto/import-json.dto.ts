import { ApiPropertyOptional } from '@nestjs/swagger'

import {
  TImportOptions,
  TImportJsonInputFormat,
  TImportJsonPayload
} from '@/core/entity/import-export/import.types'

export class ImportJsonDto {
  @ApiPropertyOptional()
  parentId?: string

  @ApiPropertyOptional({ default: {} })
  data?: TImportJsonPayload | string

  @ApiPropertyOptional({
    enum: ['json', 'jsonl', 'ndjson'],
    description: 'Optional hint for text payload parsing when `data` is a string.'
  })
  format?: TImportJsonInputFormat

  @ApiPropertyOptional({ default: '' })
  label?: string

  @ApiPropertyOptional({ default: {} })
  options?: TImportOptions
}
