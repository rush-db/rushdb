import { Controller, Get, UseInterceptors } from '@nestjs/common'

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { NeogmaDataInterceptor } from '@/database/neogma/neogma-data.interceptor'

@Controller('')
@UseInterceptors(TransformResponseInterceptor, NotFoundInterceptor, NeogmaDataInterceptor)
export class AppController {
  @Get('')
  root(): string {
    return 'RushDB is running'
  }
}
