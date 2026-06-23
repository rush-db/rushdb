import { ApiPropertyOptional } from '@nestjs/swagger'
import { ApiProperty as ApiModelProperty } from '@nestjs/swagger'
export class UpdatePropertyDto {
  @ApiPropertyOptional()
  @ApiModelProperty({ example: 'NewPropertyName' })
  name?: string

  @ApiPropertyOptional()
  @ApiModelProperty({ example: 'string' })
  type?: 'string'
}
