import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'
import { ApiModelProperty } from '@nestjs/swagger/dist/decorators/api-model-property.decorator'
import { IsNotEmpty, IsOptional, IsString, MaxLength } from 'class-validator'

import { TProjectCustomDbPayload } from '@/dashboard/project/project.types'

export class CreateProjectDto {
  @IsNotEmpty()
  @IsString()
  @MaxLength(256)
  @ApiProperty()
  @ApiModelProperty({ example: 'Reddit' })
  name: string

  @IsOptional()
  @IsString()
  @MaxLength(1024)
  @ApiPropertyOptional({ example: 'Simple forum' })
  description: string

  @ApiPropertyOptional({
    example: {
      url: 'bolt://custom-neo4j-host:7687',
      username: 'neo4j',
      password: 'password'
    }
  })
  customDb: TProjectCustomDbPayload
}
