import { BadRequestException, Injectable } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { JwtService } from '@nestjs/jwt'
import { Transaction } from 'neo4j-driver'

import { TOKEN_EXPIRES_IN } from '@/common/constants'
import { IDecodedResetToken } from '@/dashboard/auth/auth.types'
import { MailService } from '@/dashboard/mail/mail.service'
import { UserService } from '@/dashboard/user/user.service'

@Injectable()
export class EmailConfirmationService {
  constructor(
    private readonly jwtService: JwtService,
    private readonly configService: ConfigService,
    private readonly mailService: MailService,
    private readonly usersService: UserService
  ) {}

  public async confirmEmail(email: string, transaction: Transaction) {
    const user = await this.usersService.find(email, transaction).then((user) => user?.toJson())

    if (user?.confirmed) {
      throw new BadRequestException('Email already confirmed')
    }

    return await this.usersService.update(
      user.id,
      {
        confirmed: true
      },
      transaction
    )
  }

  public async decodeConfirmationToken(token: string) {
    try {
      const payload = await this.jwtService.verify(token, {
        secret: this.configService.get('RUSHDB_AES_256_ENCRYPTION_KEY')
      })

      if (typeof payload === 'object' && 'email' in payload) {
        return payload.email
      }
      throw new BadRequestException()
    } catch (error) {
      if (error?.name === 'TokenExpiredError') {
        throw new BadRequestException('Email confirmation token expired')
      }
      throw new BadRequestException('Bad confirmation token')
    }
  }

  public async decodePasswordResetToken(token: string): Promise<IDecodedResetToken> {
    try {
      const payload = await this.jwtService.verify(token, {
        secret: this.configService.get('RUSHDB_AES_256_ENCRYPTION_KEY')
      })

      if (typeof payload === 'object' && 'email' in payload && 'id' in payload) {
        return payload
      }
      throw new BadRequestException()
    } catch (error) {
      if (error?.name === 'TokenExpiredError') {
        throw new BadRequestException('Email confirmation token expired')
      }
      throw new BadRequestException('Bad reset password token')
    }
  }

  public async sendVerificationLink(email: string, userName?: string) {
    // @TODO: Patch user node and add info about sent link
    const token = this.jwtService.sign(
      { email },
      {
        secret: this.configService.get('RUSHDB_AES_256_ENCRYPTION_KEY'),
        expiresIn: TOKEN_EXPIRES_IN
      }
    )
    await this.mailService.sendUserConfirmation(email, token, userName)

    return true
  }

  public async sendForgotPasswordLink(email: string, transaction: Transaction): Promise<boolean> {
    // @TODO: Patch user node and add info about sent link
    const user = await this.usersService.find(email, transaction).then((user) => user?.toJson())

    if (!user.id) {
      // Always send true to front that we found the user
      return true
    }

    const token = this.jwtService.sign(
      { email: user.login, id: user.id },
      {
        secret: this.configService.get('RUSHDB_AES_256_ENCRYPTION_KEY'),
        expiresIn: TOKEN_EXPIRES_IN
      }
    )
    await this.mailService.sendUserForgotPasswordLink(user.login, token)

    return true
  }

  public async resendConfirmationLink(userId: string, transaction: Transaction) {
    // @TODO: Patch user node and add info about sent link
    const user = await this.usersService.findById(userId, transaction).then((user) => user?.toJson())
    if (user.confirmed) {
      throw new BadRequestException('Email already confirmed')
    }
    await this.sendVerificationLink(user.login, user.firstName)

    return true
  }
}
