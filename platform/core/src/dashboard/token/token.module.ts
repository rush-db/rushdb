import { forwardRef, Module } from '@nestjs/common'

import { BillingClientModule } from '@/core/billing-client/billing-client.module'
import { ProjectModule } from '@/dashboard/project/project.module'
import { TokenRepository } from '@/dashboard/token/model/token.repository'
import { TokenController } from '@/dashboard/token/token.controller'
import { TokenService } from '@/dashboard/token/token.service'
import { WorkspaceModule } from '@/dashboard/workspace/workspace.module'
import { UserModule } from '@/dashboard/user/user.module'

@Module({
  imports: [
    BillingClientModule,
    forwardRef(() => WorkspaceModule),
    forwardRef(() => ProjectModule),
    forwardRef(() => UserModule)
  ],
  providers: [TokenRepository, TokenService],
  exports: [TokenRepository, TokenService],
  controllers: [TokenController]
})
export class TokenModule {}
