import { MailerService } from '@nestjs-modules/mailer'
import { Injectable } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'

// https://nest-modules.github.io/mailer/docs/mailer.html
@Injectable()
export class MailService {
  constructor(private mailerService: MailerService, private readonly configService: ConfigService) {}

  async sendUserConfirmation(email: string, token: string, userName?: string) {
    const url = `${this.configService.get('RUSHDB_DASHBOARD_URL')}/confirm_email?token=${token}`

    await this.mailerService.sendMail({
      to: email,
      subject: 'Welcome to RushDB – Confirm Your Email and Simplify Your Data Journey!',
      template: 'welcome',
      context: {
        userName,
        url
      }
    })
  }

  async sendUserForgotPasswordLink(email: string, token: string) {
    const url = `${this.configService.get('RUSHDB_DASHBOARD_URL')}/auth/forgot-password/?token=${token}`
    await this.mailerService.sendMail({
      to: email,
      subject: 'Reset Your Password for RushDB Account',
      template: 'forgot-password',
      context: {
        url
      }
    })
  }
}
