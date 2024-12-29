import { Injectable } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { GoogleRecaptchaException, GoogleRecaptchaValidator } from '@nestlab/google-recaptcha'

@Injectable()
export class CaptchaService {
  constructor(
    private readonly recaptchaValidator: GoogleRecaptchaValidator,
    private readonly configService: ConfigService
  ) {}

  async isCaptchaTokenValid(recaptchaToken: string): Promise<boolean> {
    const result = await this.recaptchaValidator.validate({
      response: recaptchaToken,
      score: this.configService.get<number>('RECAPTCHA_SCORE')
    })

    if (!result.success) {
      throw new GoogleRecaptchaException(result.errors)
    }

    return true
  }
}
