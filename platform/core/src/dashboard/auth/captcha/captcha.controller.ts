import { Controller, Get, Query, UseInterceptors } from '@nestjs/common'
import { ApiBearerAuth, ApiTags } from '@nestjs/swagger'

import { CommonResponseDecorator } from '@/common/decorators/common-response.decorator'
import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { CaptchaService } from '@/dashboard/auth/captcha/captcha.service'

@Controller('captcha')
@UseInterceptors(TransformResponseInterceptor, NotFoundInterceptor)
export class CaptchaController {
  constructor(private readonly captchaService: CaptchaService) {}

  @Get()
  @ApiTags('Captcha')
  @CommonResponseDecorator()
  @ApiBearerAuth()
  async validateCaptcha(@Query('token') token?: string): Promise<boolean> {
    return await this.captchaService.isCaptchaTokenValid(token)
  }
}
