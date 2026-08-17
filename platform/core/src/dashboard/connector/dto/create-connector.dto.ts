import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'
import { IsNotEmpty, IsObject, IsOptional, IsString } from 'class-validator'

export class CreateConnectorDto {
  @IsNotEmpty()
  @IsString()
  @ApiProperty({ example: 'Production PostgreSQL' })
  name: string

  /**
   * Database types plus any worker-registered spec id. Validation against the
   * registered catalog happens in ConnectorService (Core never hardcodes the
   * connector union), so this is intentionally not an `IsIn` over a fixed list.
   */
  @IsNotEmpty()
  @IsString()
  @ApiProperty({ example: 'hubspot' })
  type: string

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
