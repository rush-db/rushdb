import { forwardRef, MiddlewareConsumer, Module, NestModule } from '@nestjs/common'

import { AuthModule } from '@/dashboard/auth/auth.module'
import { AuthMiddleware } from '@/dashboard/auth/middlewares/auth.middleware'
import { BillingModule } from '@/dashboard/billing/billing.module'
import { MailModule } from '@/dashboard/mail/mail.module'
import { ProjectModule } from '@/dashboard/project/project.module'
import { TokenModule } from '@/dashboard/token/token.module'
import { UserModule } from '@/dashboard/user/user.module'
import { WorkspaceModule } from '@/dashboard/workspace/workspace.module'
import { DbConnectionModule } from '@/database/db-connection/db-connection.module'
import { DbContextMiddleware } from '@/database/middlewares/db-context.middleware'
import { SessionAttachMiddleware } from '@/database/session-attach.middleware'

@Module({
  imports: [
    WorkspaceModule,
    ProjectModule,
    TokenModule,
    AuthModule,
    MailModule,
    UserModule,
    BillingModule,
    forwardRef(() => DbConnectionModule)
  ],
  exports: [WorkspaceModule, ProjectModule, TokenModule, AuthModule, MailModule, UserModule, BillingModule]
})
export class DashboardModule implements NestModule {
  configure(consumer: MiddlewareConsumer) {
    consumer.apply(AuthMiddleware, DbContextMiddleware, SessionAttachMiddleware).forRoutes('*')
  }
}
