import { forwardRef, Global, Module, OnApplicationBootstrap, Logger } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'

import { toBoolean } from '@/common/utils/toBolean'
import { EntityModule } from '@/core/entity/entity.module'
import { BillingModule } from '@/dashboard/billing/billing.module'
import { ProjectModule } from '@/dashboard/project/project.module'
import { TokenModule } from '@/dashboard/token/token.module'
import { WorkspaceModule } from '@/dashboard/workspace/workspace.module'
import { NeogmaService } from '@/database/neogma/neogma.service'

import { UserRepository } from './model/user.repository'
import { UserController } from './user.controller'
import { UserService } from './user.service'

@Global()
@Module({
  imports: [
    EntityModule,
    forwardRef(() => WorkspaceModule),
    forwardRef(() => ProjectModule),
    forwardRef(() => TokenModule),
    forwardRef(() => BillingModule)
  ],
  providers: [UserRepository, UserService],
  exports: [UserRepository, UserService],
  controllers: [UserController]
})
export class UserModule implements OnApplicationBootstrap {
  constructor(
    private readonly configService: ConfigService,
    private readonly userService: UserService,
    private readonly neogmaService: NeogmaService
  ) {}

  async initUser(): Promise<void> {
    const isSelfHosted = toBoolean(this.configService.get('RUSHDB_SELF_HOSTED'))
    const adminLogin = this.configService.get('RUSHDB_LOGIN')
    const adminPassword = this.configService.get('RUSHDB_PASSWORD')
    const session = this.neogmaService.createSession('init-user')

    if (isSelfHosted && adminLogin && adminPassword) {
      const transaction = session.beginTransaction()

      try {
        const user = await this.userService.find(adminLogin, transaction)

        if (user) {
          return
        }

        Logger.log('Initializing user...')
        await this.userService.create(
          {
            login: adminLogin,
            password: adminPassword,
            confirmed: true
          },
          transaction
        )
      } catch (error) {
        Logger.log('Initializing user failed.', error)
        await transaction.rollback()
      } finally {
        Logger.log('Initializing user finished.')
        if (transaction.isOpen()) {
          await transaction.commit()
          await transaction.close()
        }
        await session.close()
      }
    }
  }

  async onApplicationBootstrap() {
    await this.initUser()
  }
}
