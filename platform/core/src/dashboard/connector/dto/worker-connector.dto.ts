import { Allow, IsNotEmpty, IsNumber, IsObject, IsOptional, IsString } from 'class-validator'

export class ConnectorCommandResultDto {
  @IsOptional()
  @IsObject()
  result?: Record<string, unknown>

  @IsOptional()
  @IsString()
  errorMessage?: string
}

export class ConnectorStatusDto {
  @IsNotEmpty()
  @IsString()
  status: string

  @IsOptional()
  @IsString()
  lastError?: string

  @IsOptional()
  lagMs?: number

  @IsOptional()
  @IsObject()
  stats?: Record<string, unknown>
}

export class ConnectorOffsetDto {
  @IsNotEmpty()
  @IsString()
  partition: string

  @Allow()
  position: Record<string, unknown> | string | number | boolean | null
}

export class ConnectorHeartbeatDto {
  @IsOptional()
  @IsNumber()
  leaseTtlMs?: number

  @IsOptional()
  @IsNumber()
  lagMs?: number

  @IsOptional()
  @IsObject()
  stats?: Record<string, unknown>
}
