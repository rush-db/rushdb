import { Module } from '@nestjs/common'

import { AuthModule } from '@/dashboard/auth/auth.module'
import { BillingModule } from '@/dashboard/billing/billing.module'
import { MailModule } from '@/dashboard/mail/mail.module'
import { ProjectModule } from '@/dashboard/project/project.module'
import { TokenModule } from '@/dashboard/token/token.module'
import { UserModule } from '@/dashboard/user/user.module'
import { WorkspaceModule } from '@/dashboard/workspace/workspace.module'

@Module({
  imports: [WorkspaceModule, ProjectModule, TokenModule, AuthModule, MailModule, UserModule, BillingModule],
  exports: [WorkspaceModule, ProjectModule, TokenModule, AuthModule, MailModule, UserModule, BillingModule]
})
export class DashboardModule {}
