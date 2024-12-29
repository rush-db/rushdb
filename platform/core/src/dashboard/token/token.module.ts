import { forwardRef, Module } from '@nestjs/common'

import { ProjectModule } from '@/dashboard/project/project.module'
import { TokenRepository } from '@/dashboard/token/model/token.repository'
import { TokenQueryService } from '@/dashboard/token/token-query.service'
import { TokenController } from '@/dashboard/token/token.controller'
import { TokenService } from '@/dashboard/token/token.service'

@Module({
  imports: [forwardRef(() => ProjectModule)],
  providers: [TokenRepository, TokenService, TokenQueryService],
  exports: [TokenRepository, TokenService, TokenQueryService],
  controllers: [TokenController]
})
export class TokenModule {}
