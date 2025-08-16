import { forwardRef, Global, Module } from '@nestjs/common'

import { TransactionController } from '@/core/transactions/transaction.controller'
import { TransactionService } from '@/core/transactions/transaction.service'
import { ProjectModule } from '@/dashboard/project/project.module'
import { TokenModule } from '@/dashboard/token/token.module'
import { DbConnectionModule } from '@/database/db-connection/db-connection.module'
import { NeogmaDynamicModule } from '@/database/neogma-dynamic/neogma-dynamic.module'

@Global()
@Module({
  imports: [
    // Dashboard modules
    forwardRef(() => TokenModule),
    forwardRef(() => ProjectModule),

    //db modules
    forwardRef(() => NeogmaDynamicModule),
    forwardRef(() => DbConnectionModule)
  ],
  providers: [TransactionService],
  exports: [TransactionService],
  controllers: [TransactionController]
})
export class TransactionModule {}
