import { ApiProperty } from '@nestjs/swagger'
import { ApiModelProperty } from '@nestjs/swagger/dist/decorators/api-model-property.decorator'
import { IsNotEmpty } from 'class-validator'

export class CreateWorkspaceDto {
  @IsNotEmpty()
  @ApiProperty()
  @ApiModelProperty()
  name: string
}
