import { IsEmail } from 'class-validator'

export class RemovePendingInviteDto {
  @IsEmail()
  email!: string
}
