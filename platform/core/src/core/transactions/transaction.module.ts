import { forwardRef, Global, Module } from '@nestjs/common'

import { AiModule } from '@/core/ai/ai.module'
import { RelationshipPatternsModule } from '@/core/relationship-patterns/relationship-patterns.module'
import { TransactionController } from '@/core/transactions/transaction.controller'
import { TransactionService } from '@/core/transactions/transaction.service'
import { ProjectModule } from '@/dashboard/project/project.module'
import { TokenModule } from '@/dashboard/token/token.module'

@Global()
@Module({
  imports: [
    // Dashboard modules
    forwardRef(() => TokenModule),
    forwardRef(() => ProjectModule),

    // Needed by the commit route's RunSideEffectMixin (schema recompute +
    // relationship automation for side effects deferred from tx-wrapped writes).
    forwardRef(() => AiModule),
    forwardRef(() => RelationshipPatternsModule)
  ],
  providers: [TransactionService],
  exports: [TransactionService],
  controllers: [TransactionController]
})
export class TransactionModule {}
