import { ApiProperty, ApiPropertyOptional } from '@nestjs/swagger'
import { ApiModelProperty } from '@nestjs/swagger/dist/decorators/api-model-property.decorator'
import { IsNotEmpty } from 'class-validator'

export class CreateTokenDto {
  @IsNotEmpty()
  @ApiProperty()
  @ApiModelProperty({ example: 'PhotoApp Token' })
  name: string

  @ApiPropertyOptional({ example: 'PhotoApp Token' })
  description: string

  @IsNotEmpty()
  @ApiProperty({ example: '30d' })
  expiration: string | '*'
}

export class VerifyTokenDto {
  @IsNotEmpty()
  @ApiProperty()
  token: string
}
