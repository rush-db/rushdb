import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'
import { ApiModelProperty } from '@nestjs/swagger/dist/decorators/api-model-property.decorator'
import { IsNotEmpty } from 'class-validator'

export class CreateProjectDto {
  @IsNotEmpty()
  @ApiProperty()
  @ApiModelProperty({ example: 'Reddit' })
  name: string

  @ApiPropertyOptional({ example: 'Simple forum' })
  description: string
}
