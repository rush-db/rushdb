import { Module } from '@nestjs/common'
import { ConfigModule, ConfigService } from '@nestjs/config'
import { JwtModule } from '@nestjs/jwt'

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
        secret: configService.get<string>('JWT_SECRET'),
        signOptions: {
          expiresIn: configService.get<string>('JWT_EXPIRES_IN')
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
