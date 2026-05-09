import { forwardRef, Module } from '@nestjs/common'

import { ProjectModule } from '@/dashboard/project/project.module'
import { TokenRepository } from '@/dashboard/token/model/token.repository'
import { TokenController } from '@/dashboard/token/token.controller'
import { TokenService } from '@/dashboard/token/token.service'
import { UserModule } from '@/dashboard/user/user.module'
import { WorkspaceModule } from '@/dashboard/workspace/workspace.module'

@Module({
  imports: [forwardRef(() => WorkspaceModule), forwardRef(() => ProjectModule), forwardRef(() => UserModule)],
  providers: [TokenRepository, TokenService],
  exports: [TokenRepository, TokenService],
  controllers: [TokenController]
})
export class TokenModule {}
