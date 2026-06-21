import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'
import { IsIn, IsNotEmpty, IsObject, IsOptional, IsString } from 'class-validator'

import { CONNECTOR_TYPES, ConnectorType } from '@/dashboard/connector/connector.types'

export class CreateConnectorDto {
  @IsNotEmpty()
  @IsString()
  @ApiProperty({ example: 'Production PostgreSQL' })
  name: string

  @IsIn(CONNECTOR_TYPES)
  @ApiProperty({ enum: CONNECTOR_TYPES })
  type: ConnectorType

  @IsObject()
  @ApiProperty({
    example: {
      host: 'db.example.com',
      port: 5432,
      database: 'app',
      user: 'synx',
      tables: ['public.users', 'public.orders'],
      snapshot: true
    }
  })
  config: Record<string, unknown>

  @IsOptional()
  @IsObject()
  @ApiPropertyOptional({ example: { password: 'write-only' } })
  secrets?: Record<string, unknown>

  @IsOptional()
  @IsObject()
  @ApiPropertyOptional({ example: { fields: { ignore: ['email', 'payment.**'] } } })
  transform?: Record<string, unknown>
}

export class UpdateConnectorDto {
  @IsOptional()
  @IsString()
  @ApiPropertyOptional()
  name?: string

  @IsOptional()
  @IsObject()
  @ApiPropertyOptional()
  config?: Record<string, unknown>

  @IsOptional()
  @IsObject()
  @ApiPropertyOptional()
  secrets?: Record<string, unknown>

  @IsOptional()
  @IsObject()
  @ApiPropertyOptional()
  transform?: Record<string, unknown>
}
