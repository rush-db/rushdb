import { Module } from '@nestjs/common'

import { AuthModule } from '@/dashboard/auth/auth.module'
import { OAuthController } from '@/dashboard/mcp-oauth/controllers/oauth.controller'
import { WellKnownController } from '@/dashboard/mcp-oauth/controllers/well-known.controller'
import { McpOauthScheduler } from '@/dashboard/mcp-oauth/mcp-oauth.scheduler'
import { McpOauthService } from '@/dashboard/mcp-oauth/mcp-oauth.service'
import { OAuthRepository } from '@/dashboard/mcp-oauth/model/oauth.repository'
import { ProjectModule } from '@/dashboard/project/project.module'
import { TokenRepository } from '@/dashboard/token/model/token.repository'
import { TokenModule } from '@/dashboard/token/token.module'

@Module({
  imports: [
    AuthModule, // provides JwtService (@Global) + AuthService
    TokenModule, // provides TokenService + TokenRepository
    ProjectModule // provides ProjectService
  ],
  controllers: [WellKnownController, OAuthController],
  providers: [McpOauthService, McpOauthScheduler, OAuthRepository],
  exports: [McpOauthService]
})
export class McpOauthModule {}
