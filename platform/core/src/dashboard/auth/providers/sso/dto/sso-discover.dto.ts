import { ApiProperty } from '@nestjs/swagger'
import { IsEmail } from 'class-validator'

export class SsoDiscoverDto {
  @IsEmail()
  @ApiProperty()
  email: string
}
