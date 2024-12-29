import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'
import { ApiModelProperty } from '@nestjs/swagger/dist/decorators/api-model-property.decorator'

// @TODO: Optional Props and more options
export class UpdateProjectDto {
  @ApiProperty()
  @ApiModelProperty({ example: 'Booking.com' })
  name: string

  @ApiPropertyOptional({ example: 'Simple booking app' })
  description: string
}
