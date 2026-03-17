import { Module } from '@nestjs/common'

import { AuthModule } from '@/dashboard/auth/auth.module'
import { TokenModule } from '@/dashboard/token/token.module'
import { ProjectModule } from '@/dashboard/project/project.module'
import { WellKnownController } from '@/dashboard/mcp-oauth/controllers/well-known.controller'
import { OAuthController } from '@/dashboard/mcp-oauth/controllers/oauth.controller'
import { McpOauthService } from '@/dashboard/mcp-oauth/mcp-oauth.service'
import { McpOauthScheduler } from '@/dashboard/mcp-oauth/mcp-oauth.scheduler'
import { OAuthRepository } from '@/dashboard/mcp-oauth/model/oauth.repository'
import { TokenRepository } from '@/dashboard/token/model/token.repository'

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
