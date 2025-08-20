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
        subject: 'Welcome to RushDB – Confirm Your Email',
        template: 'welcome',
        context: {
          userName,
          url
        }
      })
    }
  }

  async sendUserInvite({
    login,
    token,
    senderName,
    workspaceName
  }: {
    login: string
    token: string
    senderName?: string
    workspaceName?: string
  }) {
    if (isEmail(login)) {
      const url = `${this.configService.get('RUSHDB_DASHBOARD_URL')}/join-workspace?invite=${encodeURIComponent(token)}`

      await this.mailerService.sendMail({
        to: login,
        subject: `You’re invited to join "${workspaceName}" on RushDB`,
        template: 'accept-invite',
        context: {
          senderUserName: senderName,
          senderUserWorkspace: workspaceName,
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
        subject: 'Reset Your Password for RushDB Cloud Account',
        template: 'forgot-password',
        context: {
          url
        }
      })
    }
  }

  // TODO: Remove this after implementin automation for managed instance requests
  async notifyAdminAboutNewProject(payload: {
    projectId: string
    name: string
    region?: string
    tier?: string
    password?: string
  }) {
    const to = this.configService.get('MAIL_USER')
    await this.mailerService.sendMail({
      to,
      subject: `MANAGED INSTANCE REQUEST - ${payload.name}`,
      template: 'managed-project-notification',
      context: {
        projectId: payload.projectId,
        projectName: payload.name,
        region: payload.region,
        tier: payload.tier,
        password: payload.password,
        dashboardUrl: this.configService.get('RUSHDB_DASHBOARD_URL')
      }
    })
  }
}
