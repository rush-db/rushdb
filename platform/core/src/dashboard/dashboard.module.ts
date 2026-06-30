import { forwardRef, MiddlewareConsumer, Module, NestModule } from '@nestjs/common'

import { AuthModule } from '@/dashboard/auth/auth.module'
import { AuthMiddleware } from '@/dashboard/auth/middlewares/auth.middleware'
import { BillingModule } from '@/dashboard/billing/billing.module'
import { ConnectorModule } from '@/dashboard/connector/connector.module'
import { MailModule } from '@/dashboard/mail/mail.module'
import { McpOauthModule } from '@/dashboard/mcp-oauth/mcp-oauth.module'
import { ProjectModule } from '@/dashboard/project/project.module'
import { SavedQueryModule } from '@/dashboard/saved-query/saved-query.module'
import { TokenModule } from '@/dashboard/token/token.module'
import { UserModule } from '@/dashboard/user/user.module'
import { WorkspaceModule } from '@/dashboard/workspace/workspace.module'
import { DbConnectionModule } from '@/database/db-connection/db-connection.module'
import { DbContextMiddleware } from '@/database/middlewares/db-context.middleware'
import { SessionAndTransactionAttachMiddleware } from '@/database/session-and-transaction-attach-middleware.service'

@Module({
  imports: [
    WorkspaceModule,
    ProjectModule,
    TokenModule,
    AuthModule,
    MailModule,
    UserModule,
    BillingModule,
    ConnectorModule,
    SavedQueryModule,
    McpOauthModule,
    forwardRef(() => DbConnectionModule)
  ],
  exports: [
    WorkspaceModule,
    ProjectModule,
    TokenModule,
    AuthModule,
    MailModule,
    UserModule,
    BillingModule,
    ConnectorModule
  ]
})
export class DashboardModule implements NestModule {
  configure(consumer: MiddlewareConsumer) {
    consumer.apply(AuthMiddleware, DbContextMiddleware, SessionAndTransactionAttachMiddleware).forRoutes('*')
  }
}
