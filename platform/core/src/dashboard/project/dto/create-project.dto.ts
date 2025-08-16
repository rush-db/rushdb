import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'
import { ApiModelProperty } from '@nestjs/swagger/dist/decorators/api-model-property.decorator'
import { IsNotEmpty } from 'class-validator'

import { TProjectCustomDbPayload } from '@/dashboard/project/project.types'

export class CreateProjectDto {
  @IsNotEmpty()
  @ApiProperty()
  @ApiModelProperty({ example: 'Reddit' })
  name: string

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

  @ApiPropertyOptional()
  managedDbConfig: {
    password: string
    region: string
    tier: string
  }
}
