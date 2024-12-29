import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'
import { IsNotEmpty } from 'class-validator'

import { TPropertyType, TPropertyValue } from '@/core/property/property.types'

export class PropertyDto {
  @ApiPropertyOptional()
  id?: string

  @IsNotEmpty()
  @ApiProperty()
  name: string

  @IsNotEmpty()
  @ApiProperty()
  type: TPropertyType

  @ApiPropertyOptional()
  value?: TPropertyValue

  @ApiPropertyOptional()
  valueSeparator?: string

  @ApiPropertyOptional()
  metadata?: string
}
