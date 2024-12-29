import { Module, DynamicModule, Provider } from '@nestjs/common'
import { ConfigModule } from '@nestjs/config'

import { RepositoryModule } from '@/database/neogma/repository/repository.module'

import { INeogmaConfig } from './neogma-config.interface'
import { NEOGMA_CONFIG, NEOGMA_INSTANCE } from './neogma.constants'
import { NeogmaService } from './neogma.service'
import { createInstance } from './neogma.util'

@Module({
  imports: [RepositoryModule],
  exports: [RepositoryModule]
})
export class NeogmaModule {
  constructor(readonly neogmaService: NeogmaService) {}

  static forRootAsync(configProvider): DynamicModule {
    const providers = [
      {
        provide: NEOGMA_CONFIG,
        ...configProvider
      } as Provider,
      {
        provide: NEOGMA_INSTANCE,
        inject: [NEOGMA_CONFIG],
        useFactory: async (config: INeogmaConfig) => createInstance(config)
      },
      NeogmaService
    ]
    return {
      module: NeogmaModule,
      global: true,
      imports: [ConfigModule],
      providers: [...providers],
      exports: [...providers]
    }
  }
}
