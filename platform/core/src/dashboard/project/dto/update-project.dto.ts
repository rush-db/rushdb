import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'
import { ApiModelProperty } from '@nestjs/swagger/dist/decorators/api-model-property.decorator'
import { TProjectCustomDbPayload } from '@/dashboard/project/project.types'

// @TODO: Optional Props and more options
export class UpdateProjectDto {
  @ApiProperty()
  @ApiModelProperty({ example: 'Booking.com' })
  name: string

  @ApiPropertyOptional({ example: 'Simple booking app' })
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
