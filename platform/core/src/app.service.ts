import { Injectable, Inject, OnModuleInit, Logger, Optional } from '@nestjs/common'

@Injectable()
export class AppService implements OnModuleInit {
  private internalService?: any // Optional service reference

  constructor(@Optional() @Inject('INTERNAL_SERVICE') private readonly _internalService: any) {
    this.internalService = _internalService
  }

  async onModuleInit() {
    if (this.internalService) {
      Logger.log(this.internalService.getSecretData(), 'AppService')
    } else {
      Logger.warn('InternalService is not available', 'AppService')
    }
  }
}
