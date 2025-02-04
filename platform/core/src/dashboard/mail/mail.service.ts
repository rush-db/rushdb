import { Injectable } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { MailerService } from '@nestjs-modules/mailer'
import { isEmail } from 'class-validator'

@Injectable()
export class MailService {
  constructor(
    private mailerService: MailerService,
    private readonly configService: ConfigService
  ) {}

  async sendUserConfirmation(login: string, token: string, userName?: string) {
    if (isEmail(login)) {
      const url = `${this.configService.get('RUSHDB_DASHBOARD_URL')}/confirm_email?token=${token}`

      await this.mailerService.sendMail({
        to: login,
        subject: 'Welcome to RushDB – Confirm Your Email and Simplify Your Data Journey!',
        template: 'welcome',
        context: {
          userName,
          url
        }
      })
    }
  }

  async sendUserForgotPasswordLink(login: string, token: string) {
    if (isEmail(login)) {
      const url = `${this.configService.get('RUSHDB_DASHBOARD_URL')}/forgot-password?token=${token}`
      await this.mailerService.sendMail({
        to: login,
        subject: 'Reset Your Password for RushDB Account',
        template: 'forgot-password',
        context: {
          url
        }
      })
    }
  }
}
