import { ApiProperty } from '@nestjs/swagger'
import { IsIn, IsNotEmpty } from 'class-validator'

import { USER_ROLE_LIST } from '@/dashboard/user/interfaces/user.constants'
import { TUserRoles } from '@/dashboard/user/model/user.interface'

export class ChangeMemberRoleDto {
  @IsNotEmpty()
  @IsIn(USER_ROLE_LIST as unknown as string[])
  @ApiProperty({ enum: USER_ROLE_LIST })
  role: TUserRoles
}
