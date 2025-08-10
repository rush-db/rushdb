import { Body, Controller, Delete, Get, Headers, Patch, UseInterceptors } from '@nestjs/common'
import { ApiBearerAuth, ApiTags, ApiExcludeController } from '@nestjs/swagger'
import { Transaction } from 'neo4j-driver'

import { CommonResponseDecorator } from '@/common/decorators/common-response.decorator'
import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { AuthService } from '@/dashboard/auth/auth.service'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { AuthUser } from '@/dashboard/user/decorators/user.decorator'
import { GetUserDto } from '@/dashboard/user/dto/get-user.dto'
import {
  IAuthenticatedUser,
  IAuthenticatedUserWithAccess
} from '@/dashboard/user/interfaces/authenticated-user.interface'
import { TransactionDecorator } from '@/database/neogma/transaction.decorator'

import { UpdateUserDto } from './dto/update-user.dto'
import { IUserClaims } from './interfaces/user-claims.interface'
import { UserService } from './user.service'

@Controller('user')
@ApiExcludeController()
@UseInterceptors(TransformResponseInterceptor, NotFoundInterceptor)
export class UserController {
  constructor(
    private readonly authService: AuthService,
    private readonly userService: UserService
  ) {}

  @Get()
  @ApiTags('Users')
  @ApiBearerAuth()
  @CommonResponseDecorator(GetUserDto)
  @AuthGuard('workspace')
  async getUser(
    @Headers() headers,
    @AuthUser() user: IUserClaims,
    @TransactionDecorator() transaction: Transaction
  ): Promise<IAuthenticatedUserWithAccess> {
    const workspaceId = headers['x-workspace-id']

    if (!workspaceId) {
      return
    }

    const userEntity = await this.userService.find(user.login, transaction)
    const userRole = await this.userService.getUserWorkspaceRole(user.login, workspaceId, transaction)
    const userData = userEntity.toJson()

    return {
      ...userData,
      token: this.authService.createToken(userEntity),
      currentScope: {
        role: userRole
      }
    }
  }

  @Patch()
  @ApiBearerAuth()
  @AuthGuard(null)
  @ApiTags('Users')
  @CommonResponseDecorator(GetUserDto)
  async patchUser(
    @AuthUser() user: IUserClaims,
    @Body() properties: UpdateUserDto,
    @TransactionDecorator() transaction: Transaction
  ): Promise<IAuthenticatedUser> {
    const updated = await this.userService.update(user.id, properties, transaction)
    const userData = updated.toJson()

    return {
      ...userData,
      token: this.authService.createToken(updated)
    }
  }

  @Delete()
  @ApiBearerAuth()
  @AuthGuard(null)
  @ApiTags('Users')
  @CommonResponseDecorator()
  async deleteUser(
    @AuthUser() user: IUserClaims,
    @TransactionDecorator() transaction: Transaction
  ): Promise<boolean> {
    const { id: userId } = user
    return await this.userService.delete({ userId, transaction })
  }
}
