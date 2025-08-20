import { forwardRef, Global, Module } from '@nestjs/common'

import { TransactionController } from '@/core/transactions/transaction.controller'
import { TransactionService } from '@/core/transactions/transaction.service'
import { ProjectModule } from '@/dashboard/project/project.module'
import { TokenModule } from '@/dashboard/token/token.module'

@Global()
@Module({
  imports: [
    // Dashboard modules
    forwardRef(() => TokenModule),
    forwardRef(() => ProjectModule)
  ],
  providers: [TransactionService],
  exports: [TransactionService],
  controllers: [TransactionController]
})
export class TransactionModule {}
