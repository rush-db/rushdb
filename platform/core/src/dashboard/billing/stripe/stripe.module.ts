import { forwardRef, Module } from '@nestjs/common'

import { TransactionModule } from '@/core/transactions/transaction.module'
import { ProjectModule } from '@/dashboard/project/project.module'
import { TokenModule } from '@/dashboard/token/token.module'
import { WorkspaceModule } from '@/dashboard/workspace/workspace.module'

import { StripeController } from './stripe.controller'
import { StripeService } from './stripe.service'

@Module({
  imports: [
    forwardRef(() => WorkspaceModule),
    forwardRef(() => ProjectModule),
    forwardRef(() => TokenModule),
    forwardRef(() => TransactionModule)
  ],
  controllers: [StripeController],
  providers: [StripeService],
  exports: [StripeService]
})
export class StripeModule {}
