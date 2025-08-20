import { Global, Module } from '@nestjs/common'
import { ConfigModule, ConfigService } from '@nestjs/config'
import { JwtModule } from '@nestjs/jwt'

import { TOKEN_EXPIRES_IN } from '@/common/constants'
import { EntityModule } from '@/core/entity/entity.module'
import { CaptchaModule } from '@/dashboard/auth/captcha/captcha.module'
import { GithubOAuthController } from '@/dashboard/auth/providers/github/github.controller'
import { GithubOAuthService } from '@/dashboard/auth/providers/github/github.service'
import { GoogleOAuthController } from '@/dashboard/auth/providers/google/google.controller'
import { GoogleOAuthService } from '@/dashboard/auth/providers/google/google.service'
import { ProjectModule } from '@/dashboard/project/project.module'
import { TokenModule } from '@/dashboard/token/token.module'
import { UserModule } from '@/dashboard/user/user.module'

import { AuthController } from './auth.controller'
import { AuthService } from './auth.service'
import { EmailConfirmationService } from './email-confirmation/email-confirmation.service'
import { EncryptionService } from './encryption/encryption.service'

@Global()
@Module({
  imports: [
    JwtModule.registerAsync({
      imports: [ConfigModule],
      inject: [ConfigService],
      useFactory: (configService: ConfigService) => ({
        secret: configService.get<string>('RUSHDB_AES_256_ENCRYPTION_KEY'),
        signOptions: {
          expiresIn: TOKEN_EXPIRES_IN
        }
      })
    }),
    TokenModule,
    ProjectModule,
    UserModule,
    EntityModule,
    CaptchaModule
  ],
  providers: [
    EncryptionService,
    AuthService,
    EmailConfirmationService,
    GoogleOAuthService,
    GithubOAuthService
  ],
  controllers: [AuthController, GoogleOAuthController, GithubOAuthController],
  exports: [EncryptionService, AuthService, CaptchaModule]
})
export class AuthModule {}
