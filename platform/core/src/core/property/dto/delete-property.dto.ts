import { ApiPropertyOptional } from '@nestjs/swagger'
import { ApiModelProperty } from '@nestjs/swagger/dist/decorators/api-model-property.decorator'

export class DeletePropertyDto {
  @ApiPropertyOptional()
  @ApiModelProperty()
  entityIds?: string[]
}
