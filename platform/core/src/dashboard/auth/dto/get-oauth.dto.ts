import { ApiResponseProperty } from '@nestjs/swagger'

import { IOauthUrl } from '@/dashboard/auth/auth.types'

export class GetOauthDto implements IOauthUrl {
  @ApiResponseProperty()
  url: string
}
