import { ArrayNotEmpty, IsArray, IsString } from 'class-validator'

export class RevokeAccessDto {
  @IsArray()
  @ArrayNotEmpty()
  @IsString({ each: true })
  userIds: string[]
}
