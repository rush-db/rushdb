import { Module } from '@nestjs/common'
import { ConfigModule, ConfigService } from '@nestjs/config'
import { GoogleRecaptchaModule } from '@nestlab/google-recaptcha'

import { CaptchaController } from '@/dashboard/auth/captcha/captcha.controller'
import { CaptchaService } from '@/dashboard/auth/captcha/captcha.service'

import { IncomingMessage } from 'http'

@Module({
  imports: [
    GoogleRecaptchaModule.forRootAsync({
      imports: [ConfigModule],
      useFactory: (configService: ConfigService) => ({
        response: (request: IncomingMessage) => (request.headers['x-captcha'] || '').toString(),
        score: configService.get<number>('RECAPTCHA_SCORE'),
        secretKey: configService.get<string>('SERVICE_CAPTCHA_KEY')
      }),
      inject: [ConfigService]
    })
  ],
  controllers: [CaptchaController],
  providers: [CaptchaService],
  exports: [CaptchaService]
})
export class CaptchaModule {}