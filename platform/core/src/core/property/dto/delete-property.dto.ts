import { ApiPropertyOptional } from '@nestjs/swagger'
import { ApiProperty as ApiModelProperty } from '@nestjs/swagger'

export class DeletePropertyDto {
  @ApiPropertyOptional()
  @ApiModelProperty()
  entityIds?: string[]
}
