import { ApiPropertyOptional } from '@nestjs/swagger'

import { TImportOptions, TImportJsonPayload } from '@/core/entity/import-export/import.types'

export class ImportJsonDto {
  @ApiPropertyOptional()
  parentId?: string

  @ApiPropertyOptional({ default: {} })
  payload?: TImportJsonPayload

  @ApiPropertyOptional({ default: '' })
  label: string

  @ApiPropertyOptional({ default: {} })
  options?: TImportOptions
}
