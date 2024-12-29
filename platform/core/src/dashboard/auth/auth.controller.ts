import {
  BadRequestException,
  Body,
  Controller,
  Get,
  Param,
  Post,
  Query,
  UnauthorizedException,
  UseInterceptors
} from '@nestjs/common'
import { ApiExcludeController, ApiQuery, ApiTags } from '@nestjs/swagger'
import { Transaction } from 'neo4j-driver'

import { CommonResponseDecorator } from '@/common/decorators/common-response.decorator'
import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { ResetPasswordAuthDto } from '@/dashboard/auth/dto/reset-password-auth.dto'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { ChangeCorsInterceptor } from '@/dashboard/common/interceptors/change-cors.interceptor'
import { AuthUser } from '@/dashboard/user/decorators/user.decorator'
import { CreateUserDto } from '@/dashboard/user/dto/create-user.dto'
import { GetUserDto } from '@/dashboard/user/dto/get-user.dto'
import { IAuthenticatedUser } from '@/dashboard/user/interfaces/authenticated-user.interface'
import { IUserClaims } from '@/dashboard/user/interfaces/user-claims.interface'
import { UserService } from '@/dashboard/user/user.service'
import { NeogmaDataInterceptor } from '@/database/neogma/neogma-data.interceptor'
import { NeogmaTransactionInterceptor } from '@/database/neogma/neogma-transaction.interceptor'
import { TransactionDecorator } from '@/database/neogma/transaction.decorator'

import { AuthService } from './auth.service'
import { LoginDto } from './dto/login.dto'
import { EmailConfirmationService } from './email-confirmation/email-confirmation.service'

@Controller('auth')
@ApiExcludeController()
@UseInterceptors(
  TransformResponseInterceptor,
  NotFoundInterceptor,
  NeogmaDataInterceptor,
  NeogmaTransactionInterceptor,
  ChangeCorsInterceptor
)
export class AuthController {
  constructor(
    private readonly userService: UserService,
    private readonly authService: AuthService,
    private readonly emailConfirmationService: EmailConfirmationService
  ) {}

  @Post('register')
  @ApiTags('Auth')
  @CommonResponseDecorator(GetUserDto)
  async register(
    @Body() user: CreateUserDto,
    @TransactionDecorator() transaction: Transaction
  ): Promise<IAuthenticatedUser> {
    // @TODO: handle existing user error properly
    try {
      return this.userService.create(user, transaction).then(async ({ userData }) => {
        const createdUserData = userData.toJson()
        await this.emailConfirmationService.sendVerificationLink(
          createdUserData.login,
          createdUserData.firstName
        )
        return {
          ...createdUserData,
          token: this.authService.createToken(userData)
        }
      })
    } catch (e) {
      throw new BadRequestException('Provided email is not allowed')
    }
  }

  @Get('confirm')
  @ApiTags('Auth')
  @ApiQuery({
    name: 'token',
    required: true,
    description: "token to verify user's email",
    type: 'string'
  })
  @CommonResponseDecorator()
  async confirmEmail(
    @TransactionDecorator() transaction: Transaction,
    @Query('token') token: string
  ): Promise<IAuthenticatedUser> {
    const email = await this.emailConfirmationService.decodeConfirmationToken(token)
    const updatedUser = await this.emailConfirmationService.confirmEmail(email, transaction)

    if (!updatedUser) {
      throw new BadRequestException()
    }

    const userData = updatedUser.toJson()

    return {
      ...userData,
      token: this.authService.createToken(updatedUser)
    }
  }

  @Get('forgot-password/:email')
  @ApiTags('Auth')
  @CommonResponseDecorator()
  async getForgotPasswordEmail(
    @TransactionDecorator() transaction: Transaction,
    @Param('email') email: string
  ) {
    return this.emailConfirmationService.sendForgotPasswordLink(email, transaction)
  }

  @Post('resend-confirmation-link')
  @ApiTags('Auth')
  @AuthGuard()
  @CommonResponseDecorator()
  async resendConfirmationLink(
    @TransactionDecorator() transaction: Transaction,
    @AuthUser() user: IUserClaims
  ) {
    return await this.emailConfirmationService.resendConfirmationLink(user.id, transaction)
  }

  @Post('reset-password')
  @ApiTags('Auth')
  @CommonResponseDecorator(GetUserDto)
  async resetUserPassword(
    @Body() resetPasswordData: ResetPasswordAuthDto,
    @TransactionDecorator() transaction: Transaction
  ): Promise<IAuthenticatedUser> {
    const { userConfirmationToken } = resetPasswordData
    const decodedToken = await this.emailConfirmationService.decodePasswordResetToken(userConfirmationToken)

    const updatedUser = await this.userService.resetUserPassword(resetPasswordData, decodedToken, transaction)

    if (!updatedUser) {
      throw new UnauthorizedException()
    }

    const userData = updatedUser.toJson()

    return {
      ...userData,
      token: this.authService.createToken(updatedUser)
    }
  }

  @Post('login')
  @ApiTags('Auth')
  @CommonResponseDecorator(GetUserDto)
  async login(
    @Body() body: LoginDto,
    @TransactionDecorator() transaction: Transaction
  ): Promise<IAuthenticatedUser> {
    const { login, password } = body

    const user = await this.authService.validateUser({ login, password }, transaction)

    if (!user) {
      throw new UnauthorizedException()
    }

    const userData = user.toJson()

    return {
      ...userData,
      token: this.authService.createToken(user)
    }
  }
}
