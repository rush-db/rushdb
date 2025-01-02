import { Module } from '@nestjs/common'
import { ConfigModule, ConfigService } from '@nestjs/config'
import { JwtModule } from '@nestjs/jwt'

import { TOKEN_EXPIRES_IN } from '@/common/constants'
import { EntityModule } from '@/core/entity/entity.module'
import { GoogleOAuthController } from '@/dashboard/auth/providers/google/google.controller'
import { GoogleOAuthService } from '@/dashboard/auth/providers/google/google.service'
import { UserModule } from '@/dashboard/user/user.module'

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
    UserModule,
    EntityModule
  ],
  providers: [GoogleOAuthService],
  controllers: [GoogleOAuthController],
  exports: [GoogleOAuthService]
})
export class GoogleOAuthModule {}
