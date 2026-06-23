import { forwardRef, Global, Module } from '@nestjs/common'
import { ConfigModule, ConfigService } from '@nestjs/config'
import { JwtModule } from '@nestjs/jwt'

import { TOKEN_EXPIRES_IN } from '@/common/constants'
import { EntityModule } from '@/core/entity/entity.module'
import { CaptchaModule } from '@/dashboard/auth/captcha/captcha.module'
import { GithubOAuthController } from '@/dashboard/auth/providers/github/github.controller'
import { GithubOAuthService } from '@/dashboard/auth/providers/github/github.service'
import { GoogleOAuthController } from '@/dashboard/auth/providers/google/google.controller'
import { GoogleOAuthService } from '@/dashboard/auth/providers/google/google.service'
import { SsoAdminController } from '@/dashboard/auth/providers/sso/sso-admin.controller'
import { SsoController } from '@/dashboard/auth/providers/sso/sso.controller'
import { SsoRepository } from '@/dashboard/auth/providers/sso/sso.repository'
import { SsoService } from '@/dashboard/auth/providers/sso/sso.service'
import { ProjectModule } from '@/dashboard/project/project.module'
import { TokenModule } from '@/dashboard/token/token.module'
import { UserModule } from '@/dashboard/user/user.module'
import { WorkspaceModule } from '@/dashboard/workspace/workspace.module'

import { AuditService } from './audit/audit.service'
import { AuthController } from './auth.controller'
import { AuthService } from './auth.service'
import { EmailConfirmationService } from './email-confirmation/email-confirmation.service'
import { EncryptionService } from './encryption/encryption.service'
import { AuthMiddleware } from './middlewares/auth.middleware'

function decodePem(value?: string): string | undefined {
  if (!value) {
    return undefined
  }
  return value.replace(/\\n/g, '\n')
}

function decodeBase64Pem(value?: string): string | undefined {
  if (!value) {
    return undefined
  }
  try {
    return decodePem(Buffer.from(value, 'base64').toString('utf8'))
  } catch {
    return undefined
  }
}

@Global()
@Module({
  imports: [
    JwtModule.registerAsync({
      imports: [ConfigModule],
      inject: [ConfigService],
      useFactory: (configService: ConfigService) => {
        const privateKey =
          decodePem(configService.get<string>('RUSHDB_JWT_PRIVATE_KEY')) ||
          decodeBase64Pem(configService.get<string>('RUSHDB_JWT_PRIVATE_KEY_BASE64'))
        const publicKey =
          decodePem(configService.get<string>('RUSHDB_JWT_PUBLIC_KEY')) ||
          decodeBase64Pem(configService.get<string>('RUSHDB_JWT_PUBLIC_KEY_BASE64'))
        const keyid = configService.get<string>('RUSHDB_JWT_KID')

        // Prefer RS256 for OAuth tokens when keys are provided; keep HS256 fallback
        // for local/self-hosted setups that still rely on the shared secret.
        if (privateKey) {
          return {
            privateKey,
            publicKey,
            signOptions: {
              expiresIn: TOKEN_EXPIRES_IN,
              algorithm: 'RS256',
              ...(keyid ? { keyid } : {})
            }
          }
        }

        return {
          secret: configService.get<string>('RUSHDB_AES_256_ENCRYPTION_KEY'),
          signOptions: {
            expiresIn: TOKEN_EXPIRES_IN
          }
        }
      }
    }),
    TokenModule,
    ProjectModule,
    UserModule,
    EntityModule,
    CaptchaModule,
    forwardRef(() => WorkspaceModule)
  ],
  providers: [
    EncryptionService,
    AuthService,
    EmailConfirmationService,
    GoogleOAuthService,
    GithubOAuthService,
    SsoService,
    SsoRepository,
    AuditService,
    AuthMiddleware
  ],
  controllers: [
    AuthController,
    GoogleOAuthController,
    GithubOAuthController,
    SsoController,
    SsoAdminController
  ],
  exports: [EncryptionService, AuthService, AuthMiddleware, CaptchaModule, JwtModule]
})
export class AuthModule {}
