import { ApiResponseProperty } from '@nestjs/swagger'
import { IsString } from 'class-validator'

import { IAuthenticatedUser } from '@/dashboard/user/interfaces/authenticated-user.interface'
import { USER_ROLE_LIST, USER_STATUS_LIST } from '@/dashboard/user/interfaces/user.constants'
import { TUserRoles, TUserStatuses } from '@/dashboard/user/model/user.interface'

export class GetUserDto implements Omit<IAuthenticatedUser, 'isEmail'> {
  @ApiResponseProperty()
  id: string

  @ApiResponseProperty()
  @IsString()
  login: string

  @ApiResponseProperty()
  created: string

  @ApiResponseProperty()
  deletedDate: string

  @ApiResponseProperty()
  edited: string

  @ApiResponseProperty()
  confirmed: boolean

  @ApiResponseProperty()
  firstName: string

  @ApiResponseProperty()
  lastActivity: string

  @ApiResponseProperty()
  lastName: string

  @ApiResponseProperty()
  paidTill: string

  @ApiResponseProperty()
  premium: boolean

  @ApiResponseProperty({
    enum: USER_ROLE_LIST
  })
  role: TUserRoles

  @ApiResponseProperty({
    enum: USER_STATUS_LIST
  })
  status: TUserStatuses

  @ApiResponseProperty()
  token: string
}
